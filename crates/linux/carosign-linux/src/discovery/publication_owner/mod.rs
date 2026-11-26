//! Preparation of a signed installation with two independent policies.
//!
//! Preparation does not publish a record and does not issue permission to launch: the stable
//! owner of the publication and the serialization of the fixation with a review are not yet
//! connected.
#![allow(dead_code)]

pub use super::verified_installation::{
    InstallationMetadataDecoder, MetadataDecodeError, ParsedInstallationManifest,
    RecoveryCheckedFiles, RecoveryFileCandidate, RecoveryFilesRefusal, check_recovery_files,
};

mod directory_metadata;
mod duplicate_fd;
mod initial_identities;
mod preparation_error;
mod prepared_publication;
mod provenance_snapshot;
mod publication_file_role;
mod publication_record_fields;
mod quarantine_inputs;
mod quarantine_preparation;
mod quarantine_refusal;
mod recovery;
mod recovery_expected;
mod reverified_provenance;
mod reverify_provenance_inputs;
mod stage_policy;
mod stage_provenance;
mod staging_location;

use preparation_error::PreparationError;
pub(super) use prepared_publication::STAGED_POLICY_PATH;
use prepared_publication::{PreparedPublication, prepare_publication};
pub use publication_file_role::PublicationFileRole;
pub use publication_record_fields::PublicationRecordFields;
pub use quarantine_inputs::QuarantineInputs;
pub use quarantine_preparation::{QuarantinePreparation, prepare_quarantine};
pub use quarantine_refusal::QuarantineRefusal;
pub use recovery::{ReverifiedContents, reverify_contents};
pub use recovery_expected::RecoveryExpected;
pub use reverified_provenance::{ReverifiedProvenance, reverify_provenance};
pub use reverify_provenance_inputs::ReverifyProvenanceInputs;
use staging_location::StagingLocation;
