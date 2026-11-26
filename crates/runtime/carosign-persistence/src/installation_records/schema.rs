use redb::TableDefinition;

pub(super) const META: TableDefinition<&[u8], &[u8]> = TableDefinition::new("meta");
pub(super) const PACKAGES: TableDefinition<&[u8], &[u8]> = TableDefinition::new("packages");
pub(super) const INSTALLATIONS: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("provider_installations");
pub(super) const SCHEMA_KEY: &[u8] = b"biometric.schema";
pub(super) const SCHEMA_V4: &[u8] = b"BSC1\0\0\0\x04\0\0\0\0";
pub(super) const SCHEMA_V5: &[u8] = b"BSC1\0\0\0\x05\0\0\0\0";
// Until the policy is confirmed after publication, the usual opening and reading of
// installations rejects the entire area.
pub(super) const PUBLICATION_QUARANTINE: &[u8] = b"biometric.publication.quarantine";
pub(super) const PUBLICATION_PROVENANCE: &[u8] = b"biometric.publication.provenance";
