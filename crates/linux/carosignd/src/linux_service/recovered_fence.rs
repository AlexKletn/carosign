use std::{
    io,
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use carosign_core::ids::BootId;
use carosign_persistence::redb::backend::{DurableReleasePermit, FenceScope};

use crate::{
    provider_supervisor::{ProviderProfile, hex},
    resources::LocalResourceOwner,
};

pub(super) fn recover(
    profile: &ProviderProfile,
    resources: &LocalResourceOwner,
    boot: BootId,
) -> io::Result<()> {
    let Some(owner) = profile
        .recovery_owner()
        .filter(|_| profile.can_probe_device())
    else {
        return Ok(());
    };
    let identity = profile.identity();
    if !resources.has_recovered_fence(&identity.resources)? {
        return Ok(());
    }
    let scheduler = resources.scheduler();
    let scopes = scheduler
        .lock()
        .map_err(|_| io::Error::other("планировщик недоступен"))?
        .recovered_scopes()
        .map_err(io::Error::other)?;
    let scope = scopes
        .into_iter()
        .find(|scope| {
            scope.authenticator.id == identity.authenticator
                && scope.resources == identity.resources
        })
        .ok_or_else(|| {
            io::Error::other("сохранённое ограждение не совпадает с областью профиля")
        })?;
    confirm(owner, &scope, boot)?;
    let operation = hex(scope.operation.as_bytes());
    // SAFETY: The configured, root-protected recovery owner confirmed physical
    // quiescence of this exact scope. Its success contract excludes renewed access.
    // The scheduler continues to block the scope until the durable release commits.
    let permit = unsafe { DurableReleasePermit::from_verified_quiescence(scope) };
    scheduler
        .lock()
        .map_err(|_| io::Error::other("планировщик недоступен"))?
        .release_recovered_fence(permit)
        .map_err(io::Error::other)?;
    eprintln!("carosignd: снято восстановленное ограждение операции {operation}");
    Ok(())
}

fn confirm(owner: &Path, scope: &FenceScope, boot: BootId) -> io::Result<()> {
    let mut command = Command::new(owner);
    command
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .process_group(0)
        .args([
            "recover-resource-v1",
            "--reason",
            "unconfirmed-release",
            "--boot",
            &hex(boot.as_bytes()),
            "--operation",
            &hex(scope.operation.as_bytes()),
            "--lease",
            &hex(scope.lease.as_bytes()),
            "--worker",
            &hex(scope.worker.as_bytes()),
            "--authenticator",
            &hex(scope.authenticator.id.as_bytes()),
            "--epoch",
            &scope.authenticator.device_epoch.value().to_string(),
        ]);
    for claim in scope.resources.claims() {
        use carosign_core::resource::AccessMode;
        let mode = match claim.mode {
            AccessMode::Exclusive => "exclusive",
            AccessMode::SharedRead => "shared-read",
            AccessMode::Mediated => "mediated",
        };
        command
            .arg("--resource")
            .arg(format!("{}:{mode}", hex(claim.id.as_bytes())));
    }
    let mut child = command.spawn()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(io::Error::other(format!(
                    "владелец не подтвердил освобождение: {status}"
                )));
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            result => {
                // SAFETY: The unreaped child owns this process group.
                unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
                let _ = child.wait();
                return Err(result.err().unwrap_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "истёк срок восстановления ресурса")
                }));
            }
        }
    }
}

#[cfg(test)]
#[path = "recovered_fence.tests.rs"]
mod tests;
