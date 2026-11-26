use super::{InstallationMetadataDecoder, MetadataDecodeError, ParsedInstallationManifest};

pub(super) struct RuntimeDecoder;
impl InstallationMetadataDecoder for RuntimeDecoder {
    fn manifest(
        &self,
        text: &str,
        name: &str,
    ) -> Result<ParsedInstallationManifest, MetadataDecodeError> {
        let m = carosign_runtime::provider::packages::parse_manifest(text)
            .map_err(|_| MetadataDecodeError)?;
        let matches: Vec<_> = m.profiles.iter().filter(|p| p.id == name).collect();
        if matches.len() != 1 {
            return Err(MetadataDecodeError);
        }
        Ok(ParsedInstallationManifest {
            package: m.package.id,
            version: m.package.version,
            executable: m.worker.executable,
            profile_path: matches[0].path.clone(),
        })
    }
    fn profile(&self, text: &str) -> Result<String, MetadataDecodeError> {
        Ok(carosign_runtime::provider::packages::parse_profile(text)
            .map_err(|_| MetadataDecodeError)?
            .id)
    }
}
