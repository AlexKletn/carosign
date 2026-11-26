use super::*;

#[test]
fn quarantine_refuses_invalid_arguments_and_untrusted_store_path() {
    assert!(matches!(
        quarantine_command(&[]),
        Err(QuarantineCommandError::Refused)
    ));
    assert!(matches!(
        trusted_store_path(std::path::Path::new("relative")),
        Err(QuarantineCommandError::Refused)
    ));
    assert!(matches!(
        trusted_store_path(std::path::Path::new("/tmp")),
        Err(QuarantineCommandError::Refused)
    ));
    assert!(trusted_store_path(std::path::Path::new("/etc/passwd")).is_ok());
}

#[test]
fn system_source_missing_refuses_even_existing_directory() {
    let root = std::fs::File::open("/").unwrap().into();
    let result = prepare(
        root,
        PackageId::from_bytes([1; 16]).unwrap(),
        ProviderId::from_bytes([2; 16]).unwrap(),
        ProfileId::from_bytes([3; 16]).unwrap(),
    );
    assert!(matches!(
        result,
        Err(LaunchRefusal::InstallationUnavailable)
    ));
}
