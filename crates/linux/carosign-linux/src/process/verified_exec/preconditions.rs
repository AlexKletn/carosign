use super::LaunchError;

/// Launch conditions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preconditions {
    /// Catalogue of the package under a trusted root.
    pub trusted_root: bool,
    /// The executable file is an ordinary file, not a link.
    pub regular_file: bool,
    /// The file owner is the root.
    pub root_owned: bool,
    /// No one, except the owner and the group, writes.
    pub not_world_writable: bool,
    /// The manifesto and profiles of the same package passed the form check.
    pub manifest_ok: bool,
}

/// Checking launch conditions.
pub fn check(conditions: &Preconditions) -> Result<(), LaunchError> {
    if !conditions.trusted_root {
        return Err(LaunchError::UntrustedRoot);
    }
    if !conditions.regular_file {
        return Err(LaunchError::NotRegularFile);
    }
    if !conditions.root_owned {
        return Err(LaunchError::NotRootOwned);
    }
    if !conditions.not_world_writable {
        return Err(LaunchError::WorldWritable);
    }
    if !conditions.manifest_ok {
        return Err(LaunchError::BadManifest);
    }
    Ok(())
}
