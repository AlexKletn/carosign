use std::os::fd::BorrowedFd;
use std::path::Path;
use std::time::Duration;

use super::InstallationMetadataDecoder;

/// Checks the signed package and holds it until it is quarantined.
pub struct QuarantineInputs<'a> {
    pub signature_policy_path: &'a Path,
    pub supervisor_policy_path: &'a Path,
    pub artifact: BorrowedFd<'a>,
    pub signature: BorrowedFd<'a>,
    pub keyring: BorrowedFd<'a>,
    pub verifier: BorrowedFd<'a>,
    pub runtime: &'a [(&'a str, BorrowedFd<'a>)],
    pub decoder: &'a dyn InstallationMetadataDecoder,
    pub installation_root: BorrowedFd<'a>,
    pub staging: BorrowedFd<'a>,
    pub version_dir: &'a str,
    pub timeout: Duration,
}
