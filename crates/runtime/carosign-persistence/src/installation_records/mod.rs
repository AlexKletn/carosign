//! Verification and storage of records of installed packages and providers.
#[cfg(test)]
use prost::Message;

use crate::{
    enrollment_codec::{envelope_with, unenvelope_with},
    mutation_plan::sha256,
};
pub use carosign::storage::v1::{
    InstallationFile, InstallationFileRole, InstallationState, PackageRecord,
    ProviderInstallationRecord, RegisteredProfile,
};
use carosign::storage::v1::{StoredValue, stored_value::Value};

mod carosign;
mod check_package;
mod installation_codec;
mod invalid_installation;
mod migration;
mod package_codec;
mod package_identity;
mod package_mismatch;
mod publication;
mod record_fields;
mod revocation;
mod schema;
mod snapshot;

pub use check_package::check_package;
pub use installation_codec::{decode, encode};
pub use invalid_installation::InvalidInstallation;
pub(crate) use migration::{
    MigratableRecord, checked_v4_record_as_v3, preflight_v3_packages, prepare_v3_package,
    prepare_v3_record,
};
pub use package_codec::decode_package;
use package_identity::check_unique_package_identity;
pub use package_mismatch::PackageMismatch;
pub use publication::PublicationFailure;
#[cfg(test)]
pub(crate) use publication::{
    publish_initial_package_and_installation, stage_publication, stage_publication_v5,
};
pub(crate) use publication::{publish_initial_with_policy_check, publish_initial_with_provenance};
#[cfg(test)]
pub(crate) use revocation::{RevokeFailure, stage_revocation, stage_uninstall};
use schema::{
    INSTALLATIONS, META, PACKAGES, PUBLICATION_PROVENANCE, PUBLICATION_QUARANTINE, SCHEMA_KEY,
    SCHEMA_V4, SCHEMA_V5,
};
use snapshot::normal_v4_meta;
pub use snapshot::{
    QuarantinedSnapshot, SnapshotRefusal, read_active_snapshot, revalidate_snapshot,
};
pub(crate) use snapshot::{
    read_quarantined_snapshot, revalidate_quarantined_snapshot, validate_v4_snapshot,
};

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
