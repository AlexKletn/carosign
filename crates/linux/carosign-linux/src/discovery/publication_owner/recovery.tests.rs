use super::*;

#[test]
fn recovery_roles_reject_changed_role_path_and_duplicates() {
    let files = BTreeMap::from([
        (STAGED_POLICY_PATH.to_owned(), vec![]),
        ("package.toml".to_owned(), vec![]),
        ("profile".to_owned(), vec![]),
        ("worker".to_owned(), vec![]),
    ]);
    let mut roles = vec![
        (STAGED_POLICY_PATH.to_owned(), PublicationFileRole::Policy),
        ("package.toml".to_owned(), PublicationFileRole::Manifest),
        ("profile".to_owned(), PublicationFileRole::Profile),
        ("worker".to_owned(), PublicationFileRole::Executable),
    ];
    assert!(check_recovery_roles(&files, "profile", "worker", &[], &roles).is_ok());
    roles[3].1 = PublicationFileRole::Runtime;
    assert!(check_recovery_roles(&files, "profile", "worker", &[], &roles).is_err());
    roles[3] = ("other".to_owned(), PublicationFileRole::Executable);
    assert!(check_recovery_roles(&files, "profile", "worker", &[], &roles).is_err());
    roles[3] = ("profile".to_owned(), PublicationFileRole::Profile);
    assert!(check_recovery_roles(&files, "profile", "worker", &[], &roles).is_err());
}
