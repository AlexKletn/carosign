//! Check the stored v3 binary before allowing migration.

use std::process::Command;

use carosign_persistence::redb::backend::RedbStore;
use redb::{Database, TableDefinition};

const META: TableDefinition<&[u8], &[u8]> = TableDefinition::new("meta");

#[test]
fn guarded_start_refuses_unrecognised_metadata_under_v3_without_writes() {
    let dir = std::env::temp_dir().join(format!("carosign-v3-meta-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    // This is fake data, not the format of the future migration log.
    for (name, value) in [
        ("pending", &b"pending"[..]),
        ("staged", &b"staged"[..]),
        ("unknown", &b"opaque"[..]),
    ] {
        let db_path = dir.join(format!("{name}.redb"));
        let socket = dir.join(format!("{name}.sock"));
        let config = dir.join(format!("{name}.conf"));
        drop(RedbStore::create_v3(&db_path).unwrap());
        {
            let db = Database::open(&db_path).unwrap();
            let tx = db.begin_write().unwrap();
            {
                let mut table = tx.open_table(META).unwrap();
                table.insert(&b"fixture.unrecognised"[..], value).unwrap();
            }
            tx.commit().unwrap();
        }
        std::fs::write(
            &config,
            format!("store={}\nsocket={}\n", db_path.display(), socket.display()),
        )
        .unwrap();
        let before = std::fs::read(&db_path).unwrap();
        assert!(RedbStore::open_v3_guarded(&db_path).is_err());
        assert_eq!(std::fs::read(&db_path).unwrap(), before);
        let output = Command::new(env!("CARGO_BIN_EXE_carosignd"))
            .arg("--config")
            .arg(&config)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("неизвестный состав таблиц v4"));
        assert!(!socket.exists());
        assert_eq!(std::fs::read(&db_path).unwrap(), before);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn guarded_start_refuses_v4_installations_under_v3_without_writes() {
    // The diagram is determined by the name of the table, even if its type is changed.
    const INSTALLATIONS: TableDefinition<&str, &str> =
        TableDefinition::new("provider_installations");
    const MULTIMAP: redb::MultimapTableDefinition<&str, &str> =
        redb::MultimapTableDefinition::new("provider_installations");
    let dir = std::env::temp_dir().join(format!("carosign-v3-v4-table-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    for multimap in [false, true] {
        let db_path = dir.join(format!("store-{multimap}.redb"));
        let socket = dir.join(format!("socket-{multimap}"));
        let config = dir.join(format!("config-{multimap}"));
        drop(RedbStore::create_v3(&db_path).unwrap());
        {
            let db = Database::open(&db_path).unwrap();
            let tx = db.begin_write().unwrap();
            {
                if multimap {
                    let mut table = tx.open_multimap_table(MULTIMAP).unwrap();
                    table.insert("fixture", "preserve").unwrap();
                } else {
                    let mut table = tx.open_table(INSTALLATIONS).unwrap();
                    table.insert("fixture", "preserve").unwrap();
                }
            }
            tx.commit().unwrap();
        }
        std::fs::write(
            &config,
            format!("store={}\nsocket={}\n", db_path.display(), socket.display()),
        )
        .unwrap();
        let before = std::fs::read(&db_path).unwrap();
        assert!(RedbStore::open_v3_guarded(&db_path).is_err());
        assert_eq!(std::fs::read(&db_path).unwrap(), before);
        let output = Command::new(env!("CARGO_BIN_EXE_carosignd"))
            .arg("--config")
            .arg(&config)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("хранилище:"));
        assert!(!socket.exists());
        assert_eq!(std::fs::read(&db_path).unwrap(), before);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn guarded_open_accepts_v3_and_rejects_locked_file_and_symlink() {
    use std::os::unix::fs::symlink;
    let dir = std::env::temp_dir().join(format!("carosign-v3-guarded-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let db_path = dir.join("store.redb");
    let link_path = dir.join("link.redb");
    let store = RedbStore::create_v3(&db_path).unwrap();
    assert!(RedbStore::open_v3_guarded(&db_path).is_err());
    drop(store);
    symlink(&db_path, &link_path).unwrap();
    assert!(RedbStore::open_v3_guarded(&link_path).is_err());
    let store = RedbStore::open_v3_guarded(&db_path).unwrap();
    assert!(store.has_schema_v3().unwrap());
    assert!(RedbStore::open_v3_guarded(&db_path).is_err());
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "требует сохранённый бинарник v3 в CAROSIGN_V3_BINARY"]
fn saved_v3_refuses_v4_and_migrating_without_socket_or_write() {
    let binary = std::env::var_os("CAROSIGN_V3_BINARY")
        .expect("требуется путь к сохранённому бинарнику v3");
    check_binary(&binary);
}

#[test]
fn guarded_binary_refuses_v4_and_migrating_without_socket_or_write() {
    check_binary(std::ffi::OsStr::new(env!("CARGO_BIN_EXE_carosignd")));
}

/// Shows the DAC boundary: Prohibiting a record from an unprivileged process does not protect
/// a file from an old service running with administrator rights.
#[test]
#[ignore = "требует сохранённый v3, unshare, setpriv и отображение двух UID"]
fn dac_barrier_does_not_block_privileged_old_binary() {
    use std::os::unix::fs::PermissionsExt;

    const CHILD: &str = "CAROSIGN_DAC_BARRIER_CHILD";
    let binary = std::env::var_os("CAROSIGN_V3_BINARY")
        .expect("требуется путь к сохранённому бинарнику v3");
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new("unshare")
            .args([
                "--user",
                "--map-root-user",
                "--map-auto",
                "--mount",
                "--net",
            ])
            .arg(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "dac_barrier_does_not_block_privileged_old_binary",
                "--ignored",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("CAROSIGN_V3_BINARY", binary)
            .output()
            .expect("не удалось запустить изолированное испытание");
        assert!(
            output.status.success(),
            "испытание namespace: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let dir = std::env::temp_dir().join(format!("carosign-dac-barrier-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let db_path = dir.join("store.redb");
    let socket = dir.join("socket");
    let config = dir.join("config");
    drop(RedbStore::create_v3(&db_path).unwrap());
    {
        let db = Database::open(&db_path).unwrap();
        let tx = db.begin_write().unwrap();
        {
            let mut table = tx.open_table(META).unwrap();
            table
                .insert(&b"biometric.schema"[..], &b"BSC1\0\0\0\x04\0\0\0\0"[..])
                .unwrap();
        }
        tx.commit().unwrap();
    }
    std::fs::write(
        &config,
        format!("store={}\nsocket={}\n", db_path.display(), socket.display()),
    )
    .unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o644)).unwrap();
    std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o400)).unwrap();
    let before = std::fs::read(&db_path).unwrap();

    let can_write = Command::new("setpriv")
        .args(["--reuid=1", "--regid=1", "--clear-groups"])
        .args(["/usr/bin/test", "-w"])
        .arg(&db_path)
        .status()
        .unwrap();
    assert!(!can_write.success());

    let unprivileged = Command::new("setpriv")
        .args(["--reuid=1", "--regid=1", "--clear-groups"])
        .arg(&binary)
        .arg("--config")
        .arg(&config)
        .output()
        .unwrap();
    assert!(!unprivileged.status.success());
    assert_eq!(std::fs::read(&db_path).unwrap(), before);
    assert!(!socket.exists());

    let privileged = Command::new(&binary)
        .arg("--config")
        .arg(&config)
        .output()
        .unwrap();
    assert!(!privileged.status.success());
    assert!(!socket.exists());
    let after = std::fs::read(&db_path).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_ne!(
        after, before,
        "запуск с правами администратора не воспроизвёл запись"
    );
}

fn check_binary(binary: &std::ffi::OsStr) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "carosign-v3-check-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).unwrap();
    let mut changed = Vec::new();
    for (name, marker) in [
        ("v4", &b"BSC1\0\0\0\x04\0\0\0\0"[..]),
        ("migrating", &b"BSC1\0\0\0\x03\0\0\0\x01"[..]),
    ] {
        let db_path = dir.join(format!("{name}.redb"));
        let socket = dir.join(format!("{name}.sock"));
        let config = dir.join(format!("{name}.conf"));
        drop(RedbStore::create_v3(&db_path).unwrap());
        {
            let db = Database::open(&db_path).unwrap();
            let tx = db.begin_write().unwrap();
            {
                let mut table = tx.open_table(META).unwrap();
                table.insert(&b"biometric.schema"[..], marker).unwrap();
            }
            tx.commit().unwrap();
        }
        std::fs::write(
            &config,
            format!("store={}\nsocket={}\n", db_path.display(), socket.display()),
        )
        .unwrap();
        let before = std::fs::read(&db_path).unwrap();
        let output = Command::new(binary)
            .arg("--config")
            .arg(&config)
            .output()
            .unwrap();
        assert!(!output.status.success(), "старый процесс принял {name}");
        assert!(
            !socket.exists(),
            "старый процесс опубликовал сокет для {name}"
        );
        if std::fs::read(&db_path).unwrap() != before {
            changed.push(name);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        changed.is_empty(),
        "старая версия изменила файлы базы: {changed:?}"
    );
}
