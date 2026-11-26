//! Verified system recording with fixed installation files.
//!
//! TOML text and file measurement alone do not create confirmation; the actual source of the
//! approved record is not yet connected; the code does not allow launch.

#![allow(dead_code)]

mod approved_file;
mod approved_installation;
mod approved_path;
mod authority;
#[cfg(test)]
#[cfg(test)]
mod changed_authority;
mod checked_files;
mod file_role;
mod installation_key;
mod installation_metadata_decoder;
mod kernel_verity;
mod measure;
mod metadata_decode_error;
mod must_not_measure;
mod parsed_installation_manifest;
mod pinned_file;
mod protected;
mod recovery_checked_files;
mod recovery_file_candidate;
mod recovery_files_refusal;
mod refusal;
mod unavailable_authority;

#[cfg(test)]
mod fixture_measure;
#[cfg(test)]
mod installation_fixture;
#[cfg(test)]
mod runtime_decoder;

use approved_file::ApprovedFile;
use approved_installation::ApprovedInstallation;
use authority::Authority;
pub(super) use checked_files::{CheckedFiles, system_validate};
use file_role::FileRole;
use installation_key::InstallationKey;
pub use installation_metadata_decoder::InstallationMetadataDecoder;
use kernel_verity::KernelVerity;
use measure::Measure;
pub use metadata_decode_error::MetadataDecodeError;
pub use parsed_installation_manifest::ParsedInstallationManifest;
pub use recovery_checked_files::{RecoveryCheckedFiles, check_recovery_files};
pub use recovery_file_candidate::RecoveryFileCandidate;
pub use recovery_files_refusal::RecoveryFilesRefusal;
pub(super) use refusal::Refusal;
use unavailable_authority::UnavailableAuthority;
