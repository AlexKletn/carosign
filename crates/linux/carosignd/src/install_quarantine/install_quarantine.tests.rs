use super::*;
use carosign_core::ids::{PackageId, ProfileId, ProviderId};
use std::os::fd::AsFd;

#[test]
fn recovery_root_requires_a_protected_absolute_directory() {
    let root = TrustedRecoveryRoot::open(Path::new("/")).unwrap();
    assert!(root.revalidate().is_ok());
    assert!(TrustedRecoveryRoot::open(Path::new("etc")).is_err());
    assert!(TrustedRecoveryRoot::open(Path::new("/etc/../tmp")).is_err());
    assert!(TrustedRecoveryRoot::open(Path::new("/tmp")).is_err());
    assert!(TrustedRecoveryRoot::open(Path::new("/proc/self")).is_err());
}

#[test]
fn recovery_root_clone_rechecks_path_after_original_is_dropped() {
    let original = TrustedRecoveryRoot::open(Path::new("/")).unwrap();
    let mut retained = original.try_clone().unwrap();
    drop(original);
    assert!(retained.revalidate().is_ok());
    retained.path = Path::new("/proc/self").to_owned();
    assert!(retained.revalidate().is_err());
}

#[test]
fn fingerprint_is_strict_uppercase_hex() {
    assert_eq!(decode_fingerprint("00AF12").unwrap(), [0, 175, 18]);
    for bad in ["A", "af", "GG", "а0"] {
        assert!(decode_fingerprint(bad).is_err());
    }
}

#[test]
fn synthetic_fields_reach_durable_quarantine() {
    let fields = PublicationRecordFields {
        package: PackageId::from_bytes([1; 16]).unwrap(),
        provider: ProviderId::from_bytes([2; 16]).unwrap(),
        profile: ProfileId::from_bytes([3; 16]).unwrap(),
        policy_id: [4; 16],
        package_name: "org.example.provider".into(),
        version: "1.0".into(),
        profile_name: "default".into(),
        version_dir: "versions/one".into(),
        device: 1,
        inode: 2,
        signer_version: 4,
        signer_fingerprint: "09".repeat(20),
        profile_path: "profile".into(),
        executable_path: "worker".into(),
        files: vec![
            (
                ".carosign-supervisor-policy.toml".into(),
                PublicationFileRole::Policy,
                [8; 32],
            ),
            (
                "package.toml".into(),
                PublicationFileRole::Manifest,
                [8; 32],
            ),
            ("profile".into(), PublicationFileRole::Profile, [8; 32]),
            ("worker".into(), PublicationFileRole::Executable, [8; 32]),
        ],
        provenance_verity: [[9; 32], [10; 32]],
    };
    let (package, installation) = records(fields).unwrap();
    let key = [
        installation.package_id.clone(),
        installation.profile_id.clone(),
    ]
    .concat();
    carosign_persistence::installation_records::check_package(&key, &installation, &package)
        .unwrap();
    let path = std::env::temp_dir().join(format!(
        "carosign-quarantine-{}-{}.redb",
        std::process::id(),
        std::thread::current().name().unwrap()
    ));
    let store = RedbStore::create_current(&path).unwrap();
    assert_eq!(
        store.quarantine_initial_installation(&package, &installation, || true),
        Err(PublicationFailure::RecoveryRequired)
    );
    // The lack of measurements of origin does not allow even independent verification to
    // begin and does not change the quarantine.
    let root = TrustedRecoveryRoot::open(Path::new("/etc")).unwrap();
    let fd = std::fs::File::open("/dev/null").unwrap();
    let checked = unsafe {
        reverify_quarantine(
            &store,
            package.package_name.as_bytes(),
            &key,
            RecoveryInputs {
                root: &root,
                signature_policy_path: Path::new("/not-used"),
                supervisor_policy_path: Path::new("/not-used"),
                decoder: &crate::installation::RuntimeDecoder,
                keyring: fd.as_fd(),
                verifier: fd.as_fd(),
                runtime: &[],
                timeout: Duration::from_secs(1),
            },
        )
    };
    assert!(checked.is_err());
    drop(store);
    assert!(RedbStore::open_current_guarded(&path).is_err());
    std::fs::remove_file(path).unwrap();
}
