/// Untested fields obtained by rigorous analysis of an existing manifesto.
pub struct ParsedInstallationManifest {
    /// Canonical name of the package.
    pub package: String,
    /// The package version.
    pub version: String,
    /// The relative path of the executable file.
    pub executable: String,
    /// The relative path of the requested profile.
    pub profile_path: String,
}
