use super::{
    InstallationMetadataDecoder, MetadataDecodeError, ParsedInstallationManifest, parse_manifest,
    parse_profile,
};

pub(crate) struct RuntimeDecoder;

impl InstallationMetadataDecoder for RuntimeDecoder {
    fn manifest(
        &self,
        text: &str,
        profile: &str,
    ) -> Result<ParsedInstallationManifest, MetadataDecodeError> {
        let manifest = parse_manifest(text).map_err(|_| MetadataDecodeError)?;
        let mut matches = manifest.profiles.iter().filter(|entry| entry.id == profile);
        let path = matches.next().ok_or(MetadataDecodeError)?.path.clone();
        if matches.next().is_some() {
            return Err(MetadataDecodeError);
        }
        Ok(ParsedInstallationManifest {
            package: manifest.package.id,
            version: manifest.package.version,
            executable: manifest.worker.executable,
            profile_path: path,
        })
    }

    fn profile(&self, text: &str) -> Result<String, MetadataDecodeError> {
        Ok(parse_profile(text).map_err(|_| MetadataDecodeError)?.id)
    }
}
