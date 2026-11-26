//! Check the contents of the signed image before retrieval.
//!
//! Only uncompressed tar with regular files and directories is supported.
//! The format of the container is not specified by the specification; this internal
//! disassembler is not an installer and does not substitute for verification of origin.
#![allow(dead_code)]

mod archive_error;
mod check_policy;
mod checked_archive;
#[cfg(test)]
pub(super) mod fixture_decoder;
mod inspect;
mod inspect_and_stage_measured;
mod matches_signed_bytes;
mod safe_path;
mod stage;
mod stage_and_measure;
#[cfg(test)]
mod stage_directory;

pub(super) use archive_error::ArchiveError;
pub(super) use check_policy::check_policy;
pub(super) use checked_archive::CheckedArchive;
pub(super) use inspect::inspect;
#[cfg(test)]
use inspect::inspect_bytes;
pub(super) use inspect_and_stage_measured::inspect_policy_and_stage_measured;
pub(super) use matches_signed_bytes::matches_signed_bytes;
use safe_path::safe_path;
#[cfg(test)]
use stage::same_staged_file;
pub(super) use stage::stage;
pub(super) use stage_and_measure::stage_and_measure;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
