use crate::discovery::verified_installation::{
    InstallationMetadataDecoder, MetadataDecodeError, ParsedInstallationManifest,
};

pub(in crate::discovery) struct Decoder;
impl InstallationMetadataDecoder for Decoder {
    fn manifest(
        &self,
        _: &str,
        _: &str,
    ) -> Result<ParsedInstallationManifest, MetadataDecodeError> {
        Ok(ParsedInstallationManifest {
            package: "org.example.test".into(),
            version: "1".into(),
            executable: "bin/worker".into(),
            profile_path: "profiles/default.toml".into(),
        })
    }
    fn profile(&self, _: &str) -> Result<String, MetadataDecodeError> {
        Ok("default".into())
    }
}
