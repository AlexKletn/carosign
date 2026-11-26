use super::*;
use std::io::Write;

#[test]
fn profile_path_comes_from_retained_manifest_and_must_be_staged() {
    use crate::discovery::verified_installation::{
        MetadataDecodeError, ParsedInstallationManifest,
    };

    struct Decoder;
    impl InstallationMetadataDecoder for Decoder {
        fn manifest(
            &self,
            text: &str,
            profile: &str,
        ) -> Result<ParsedInstallationManifest, MetadataDecodeError> {
            if text != "signed manifest" || profile != "default" {
                return Err(MetadataDecodeError);
            }
            Ok(ParsedInstallationManifest {
                package: "example".into(),
                version: "1".into(),
                executable: "bin/provider".into(),
                profile_path: "profiles/default.toml".into(),
            })
        }
        fn profile(&self, _: &str) -> Result<String, MetadataDecodeError> {
            Err(MetadataDecodeError)
        }
    }

    fn file(contents: &[u8]) -> OwnedFd {
        let fd = unsafe { libc::memfd_create(c"publication-test".as_ptr(), libc::MFD_CLOEXEC) };
        assert!(fd >= 0);
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        file.write_all(contents).unwrap();
        file.into()
    }

    let mut policy = SupervisorPolicy {
        package: "example".into(),
        version: "1".into(),
        profile: "default".into(),
        architecture: "aarch64".into(),
        executable: "bin/provider".into(),
        uid: 1,
        gid: 1,
        memory_bytes: 1,
        cpu_quota_us: 1,
        cpu_period_us: 1,
        processes: 1,
        runtime: vec![],
        syscalls: vec![],
    };
    let mut files = BTreeMap::from([
        ("package.toml".into(), (file(b"signed manifest"), [0; 32])),
        ("profiles/default.toml".into(), (file(b"profile"), [0; 32])),
    ]);
    assert_eq!(
        retained_profile_path(&files, &policy, &Decoder).unwrap(),
        "profiles/default.toml"
    );
    files.remove("profiles/default.toml");
    assert!(retained_profile_path(&files, &policy, &Decoder).is_err());
    files.insert("profiles/default.toml".into(), (file(b"profile"), [0; 32]));
    policy.package = "different".into();
    assert!(retained_profile_path(&files, &policy, &Decoder).is_err());
    policy.package = "example".into();
    files.insert("package.toml".into(), (file(b"changed manifest"), [0; 32]));
    assert!(retained_profile_path(&files, &policy, &Decoder).is_err());
}

#[test]
fn staged_policy_is_separate_and_cannot_replace_archive_file() {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::temp_dir().join(format!(
        "carosign-publication-staging-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let root = std::fs::File::open(&path).unwrap();
    let file = stage_policy(root.as_fd(), b"schema=1\n").unwrap();
    assert!(policy_matches_snapshot(&file, b"schema=1\n").unwrap());
    assert!(!policy_matches_snapshot(&file, b"schema=2\n").unwrap());
    assert_eq!(
        std::fs::read(path.join(STAGED_POLICY_PATH)).unwrap(),
        b"schema=1\n"
    );
    assert_eq!(
        stage_policy(root.as_fd(), b"replacement")
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(
        std::fs::read(path.join(STAGED_POLICY_PATH)).unwrap(),
        b"schema=1\n"
    );
    drop(file);
    drop(root);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn supervisor_policy_change_stops_preparation() {
    use std::fs::OpenOptions;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open("/dev/shm/carosign-policy-tests.lock")
        .unwrap();
    assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) }, 0);
    let path = std::path::Path::new("/dev/shm").join(format!(
        "carosign-publication-policy-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::write(&path, b"policy").unwrap();
    let mut watch = PolicyChangeWatch::new(&path).unwrap();
    check_watch(&mut watch).unwrap();
    std::fs::write(&path, b"changed").unwrap();
    assert!(matches!(
        check_watch(&mut watch),
        Err(PreparationError::PolicyChanged)
    ));
    assert!(matches!(
        check_watch(&mut watch),
        Err(PreparationError::PolicyChanged)
    ));
    std::fs::remove_file(path).unwrap();
}
