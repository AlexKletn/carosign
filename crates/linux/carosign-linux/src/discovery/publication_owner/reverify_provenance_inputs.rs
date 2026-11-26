use std::os::fd::BorrowedFd;
use std::path::Path;
use std::time::Duration;

/// Fresh quarantine data and independently prepared entrances.
pub struct ReverifyProvenanceInputs<'a> {
    pub root: BorrowedFd<'a>,
    pub expected_verity: &'a [[u8; 32]; 2],
    pub signature_policy_path: &'a Path,
    pub keyring: BorrowedFd<'a>,
    pub verifier: BorrowedFd<'a>,
    pub runtime: &'a [(&'a str, BorrowedFd<'a>)],
    pub expected_policy_id: [u8; 16],
    pub expected_fingerprint: &'a str,
    pub expected_key_version: u8,
    pub timeout: Duration,
}
