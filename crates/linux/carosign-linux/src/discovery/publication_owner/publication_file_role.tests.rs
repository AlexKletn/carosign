use super::*;

#[test]
fn publication_roles_are_distinct_and_unknown_files_are_rejected() {
    let runtime = vec!["lib/runtime.so".to_owned()];
    assert_eq!(
        publication_file_role(STAGED_POLICY_PATH, "profile.toml", "bin/provider", &runtime),
        Some(PublicationFileRole::Policy)
    );
    assert_eq!(
        publication_file_role("package.toml", "profile.toml", "bin/provider", &runtime),
        Some(PublicationFileRole::Manifest)
    );
    assert_eq!(
        publication_file_role("profile.toml", "profile.toml", "bin/provider", &runtime),
        Some(PublicationFileRole::Profile)
    );
    assert_eq!(
        publication_file_role("bin/provider", "profile.toml", "bin/provider", &runtime),
        Some(PublicationFileRole::Executable)
    );
    assert_eq!(
        publication_file_role("lib/runtime.so", "profile.toml", "bin/provider", &runtime),
        Some(PublicationFileRole::Runtime)
    );
    assert_eq!(
        publication_file_role("unknown", "profile.toml", "bin/provider", &runtime),
        None
    );
    assert_eq!(
        publication_file_role("profile.toml", "profile.toml", "profile.toml", &runtime),
        None
    );
}
