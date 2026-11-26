use super::STAGED_POLICY_PATH;

/// The role of the measured file of the future record. The role itself is not an authority.
#[derive(Debug, PartialEq, Eq)]
pub enum PublicationFileRole {
    Policy,
    Manifest,
    Profile,
    Executable,
    Runtime,
}

pub(super) fn publication_file_role(
    path: &str,
    profile: &str,
    executable: &str,
    runtime: &[String],
) -> Option<PublicationFileRole> {
    let mut roles = Vec::new();
    if path == STAGED_POLICY_PATH {
        roles.push(PublicationFileRole::Policy);
    }
    if path == "package.toml" {
        roles.push(PublicationFileRole::Manifest);
    }
    if path == profile {
        roles.push(PublicationFileRole::Profile);
    }
    if path == executable {
        roles.push(PublicationFileRole::Executable);
    }
    if runtime.iter().any(|item| item == path) {
        roles.push(PublicationFileRole::Runtime);
    }
    if roles.len() == 1 { roles.pop() } else { None }
}

#[cfg(test)]
#[path = "publication_file_role.tests.rs"]
mod tests;
