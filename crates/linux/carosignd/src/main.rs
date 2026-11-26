//! The point of entry is in the library. `carosignd`.

mod boot_clock;
mod initial_file;
mod metadata_store;
mod system_defaults;

use carosign_runtime::ports::TimeSource;
use carosignd::bootstrap::{Readiness, SchemaStatus, recover_guarded};
use carosignd::config::Config;

/// A local limit on the size of the settings file; this is not a confirmation of its trust.
const CONFIG_MAX_BYTES: usize = 1024 * 1024;

/// A limited set of launch events; settings and paths are not accepted.
#[derive(Clone, Copy)]
enum StartupEvent {
    StoreRecoveryFailed,
    CoordinatorFailed,
    ClockUnavailable,
    IdSourceUnavailable,
    DependenciesUnavailable,
    RecoveryUnconfirmed,
    ServiceLoopUnavailable,
}

impl StartupEvent {
    fn code(self) -> &'static str {
        match self {
            Self::StoreRecoveryFailed => "store_recovery_failed",
            Self::CoordinatorFailed => "coordinator_failed",
            Self::ClockUnavailable => "clock_unavailable",
            Self::IdSourceUnavailable => "id_source_unavailable",
            Self::DependenciesUnavailable => "dependencies_unavailable",
            Self::RecoveryUnconfirmed => "recovery_unconfirmed",
            Self::ServiceLoopUnavailable => "service_loop_unavailable",
        }
    }
}

/// Start-up diagnostics that do not replace a separate audit stream and its storage.
fn startup_diagnostic(event: StartupEvent) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|time| time.as_secs())
        .unwrap_or(0);
    eprintln!(
        "component=carosignd phase=startup event={} outcome=unavailable timestamp_unix_s={timestamp}",
        event.code()
    );
}

fn read_config(path: &std::path::Path) -> std::io::Result<String> {
    use std::io::{Error, ErrorKind, Read};
    use std::os::unix::fs::OpenOptionsExt;

    // The privileged demon only accepts settings from a secure system path; the average user
    // can check their copy.
    if unsafe { libc::geteuid() } == 0 {
        let (bytes, mut watch) =
            carosign_linux::policy::ProtectedPolicySnapshot::open(path)?.into_parts();
        if watch.check()? != carosign_linux::policy::PolicyWatchState::Unchanged {
            return Err(Error::new(ErrorKind::InvalidData, "настройки изменились"));
        }
        return String::from_utf8(bytes)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "настройки не UTF-8"));
    }
    // Opening of FIFO without O_NONBLOCK It can stop the download forever.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::new(ErrorKind::InvalidData, "нужен обычный файл"));
    }
    let mut bytes = Vec::new();
    file.take(CONFIG_MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > CONFIG_MAX_BYTES {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "файл настроек слишком велик",
        ));
    }
    String::from_utf8(bytes).map_err(|_| Error::new(ErrorKind::InvalidData, "настройки не UTF-8"))
}

fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|arg| arg == "install-quarantine")
    {
        match carosignd::installation::quarantine_command(&arguments[1..]) {
            Ok(()) => return,
            Err(carosignd::installation::QuarantineCommandError::RecoveryRequired) => {
                eprintln!(
                    "carosignd: установка в карантине; требуется независимое восстановление"
                );
                std::process::exit(1);
            }
            Err(_) => {
                eprintln!("carosignd: карантинная установка не подтверждена");
                std::process::exit(1);
            }
        }
    }
    if arguments
        .first()
        .is_some_and(|arg| arg == "init-resource-fences")
    {
        if arguments.len() != 2 || unsafe { libc::getuid() } != 0 || unsafe { libc::geteuid() } != 0
        {
            eprintln!("использование: carosignd init-resource-fences /абсолютный/путь (root)");
            std::process::exit(2);
        }
        let path = std::path::Path::new(&arguments[1]);
        let parent = path.parent().filter(|parent| parent.is_absolute());
        if !parent.is_some_and(|parent| {
            carosignd::installation::trusted_store_path(parent).is_ok()
                && std::fs::symlink_metadata(parent).is_ok_and(|metadata| metadata.is_dir())
        }) {
            eprintln!("carosignd: каталог ограждений не защищён");
            std::process::exit(1);
        }
        match carosign_persistence::redb::backend::InitialFenceStore::create_new(path) {
            Ok(_) => return,
            Err(_) => {
                eprintln!("carosignd: не удалось создать хранилище ограждений");
                std::process::exit(1);
            }
        }
    }
    if arguments.first().is_some_and(|arg| arg == "init-store") {
        if arguments.len() != 2 {
            eprintln!("использование: carosignd init-store /абсолютный/путь");
            std::process::exit(2);
        }
        let path = std::path::Path::new(&arguments[1]);
        if unsafe { libc::geteuid() } == 0 {
            let parent = path.parent().filter(|parent| parent.is_absolute());
            if !parent.is_some_and(|parent| {
                carosignd::installation::trusted_store_path(parent).is_ok()
                    && std::fs::symlink_metadata(parent).is_ok_and(|metadata| metadata.is_dir())
            }) {
                eprintln!("carosignd: каталог базы не защищён");
                std::process::exit(1);
            }
        }
        match carosign_persistence::redb::backend::RedbStore::create_current(path) {
            Ok(_) => return,
            Err(error) => {
                eprintln!("carosignd: не удалось создать хранилище: {error}");
                std::process::exit(1);
            }
        }
    }
    let config = match arguments.as_slice() {
        [] => Config::default(),
        [option, path] if option == "--config" => {
            let path = std::path::Path::new(path);
            if !path.is_absolute() {
                eprintln!("carosignd: путь к настройкам должен быть абсолютным");
                std::process::exit(2);
            }
            let text = read_config(path).unwrap_or_else(|error| {
                eprintln!("carosignd: не удалось прочитать настройки: {error}");
                std::process::exit(1);
            });
            Config::parse(&text).unwrap_or_else(|error| {
                eprintln!("carosignd: настройки: {error}");
                std::process::exit(1);
            })
        }
        _ => {
            eprintln!("использование: carosignd [--config /абсолютный/путь]");
            std::process::exit(2);
        }
    };
    if let Err(error) = config.validate() {
        eprintln!("carosignd: {error}");
        std::process::exit(1);
    }
    if let Err(error) = metadata_store::prepare(std::path::Path::new(&config.store)) {
        startup_diagnostic(StartupEvent::StoreRecoveryFailed);
        eprintln!("carosignd: {error}");
        std::process::exit(1);
    }
    let (store, recovered, readiness) = match recover_guarded(&config.store) {
        Ok(result) => result,
        Err(error) => {
            startup_diagnostic(StartupEvent::StoreRecoveryFailed);
            eprintln!("carosignd: {error}");
            std::process::exit(1);
        }
    };
    let _recovery_coordinator = recovered.coordinator().unwrap_or_else(|error| {
        startup_diagnostic(StartupEvent::CoordinatorFailed);
        eprintln!("carosignd: {error}");
        std::process::exit(1);
    });
    let clock = std::sync::Arc::new(boot_clock::BootClock::initialize().unwrap_or_else(|error| {
        startup_diagnostic(StartupEvent::ClockUnavailable);
        eprintln!("carosignd: системные часы недоступны: {error}");
        std::process::exit(1);
    }));
    if !carosign_linux::ids::available() {
        startup_diagnostic(StartupEvent::IdSourceUnavailable);
        eprintln!("carosignd: источник случайных идентификаторов недоступен");
        std::process::exit(1);
    }
    if let Err(error) = system_defaults::prepare(&config) {
        startup_diagnostic(StartupEvent::DependenciesUnavailable);
        eprintln!("carosignd: не удалось подготовить системные настройки: {error}");
        std::process::exit(1);
    }
    let policy = carosignd::linux_policy::LinuxPolicy::open(
        std::path::Path::new(&config.policy),
        clock.clone(),
    );
    if let Ok(policy) = &policy
        && config.provider_profile.is_none()
        && unsafe { libc::geteuid() } == 0
    {
        if let Err(error) =
            carosignd::linux_service::serve_discovered(&config, &store, policy, clock.clone())
        {
            eprintln!("carosignd: служба остановлена: {error}");
        }
        std::process::exit(1);
    }
    if let (Ok(policy), Some(path)) = (&policy, config.provider_profile.as_deref())
        && unsafe { libc::geteuid() } == 0
    {
        match carosignd::provider_supervisor::ProviderProfile::open(std::path::Path::new(path)) {
            Ok(profile) => {
                // Check the launch and private channel before the socket is published.
                let started = carosignd::provider_supervisor::ProviderSupervisor::start(
                    &profile,
                    *clock.boot_id().as_bytes(),
                    carosign_core::device::DeviceEpoch::FIRST,
                    std::time::Duration::from_secs(5),
                );
                if let Ok(mut supervisor) = started {
                    let resources = carosignd::resources::LocalResourceOwner::default();
                    if supervisor.stop_and_confirm(&resources).is_ok() {
                        if let Err(error) = carosignd::linux_service::serve(
                            &config,
                            &store,
                            policy,
                            &profile,
                            clock.clone(),
                        ) {
                            eprintln!("carosignd: служба остановлена: {error}");
                        }
                        std::process::exit(1);
                    }
                }
                eprintln!("carosignd: проверка запуска провайдера не прошла");
            }
            Err(_) => eprintln!("carosignd: системный профиль провайдера недоступен"),
        }
    }
    match readiness {
        Readiness::NotReady if recovered.schema == SchemaStatus::V4 => {
            startup_diagnostic(StartupEvent::DependenciesUnavailable);
            eprintln!("carosignd: обязательные порты службы ещё не подключены");
            if policy.is_err() {
                eprintln!("carosignd: системная политика недоступна или не прошла проверку");
            }
            eprintln!("carosignd: проверьте профиль провайдера и восстановление");
        }
        Readiness::NotReady => {
            startup_diagnostic(StartupEvent::RecoveryUnconfirmed);
            eprintln!("carosignd: восстановление не подтверждено");
        }
        Readiness::Ready | Readiness::Degraded => {
            startup_diagnostic(StartupEvent::ServiceLoopUnavailable);
            eprintln!("carosignd: цикл службы ещё не подключён")
        }
    }
    std::process::exit(1);
}
