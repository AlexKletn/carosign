use super::PublicationFileRole;

/// Metadata quarantine record, which must be associated with the archive.
pub struct RecoveryExpected<'a> {
    pub package: &'a str,
    pub version: &'a str,
    pub profile: &'a str,
    pub profile_path: &'a str,
    pub executable_path: &'a str,
    pub files: &'a [(String, PublicationFileRole)],
}
