use super::{MetadataDecodeError, ParsedInstallationManifest};

/// The parser creates a trusted service; the text does not create authority.
pub trait InstallationMetadataDecoder {
    /// Strictly disassembles the manifesto and selects exactly one specified profile.
    fn manifest(
        &self,
        text: &str,
        profile: &str,
    ) -> Result<ParsedInstallationManifest, MetadataDecodeError>;
    /// Strictly disassembles the profile and returns its name.
    fn profile(&self, text: &str) -> Result<String, MetadataDecodeError>;
}
