use super::{BorrowedFd, Duration, InstallationMetadataDecoder, Path, TrustedRecoveryRoot};

pub(crate) struct RecoveryInputs<'a> {
    pub(crate) root: &'a TrustedRecoveryRoot,
    pub(crate) signature_policy_path: &'a Path,
    pub(crate) supervisor_policy_path: &'a Path,
    pub(crate) decoder: &'a dyn InstallationMetadataDecoder,
    pub(crate) keyring: BorrowedFd<'a>,
    pub(crate) verifier: BorrowedFd<'a>,
    pub(crate) runtime: &'a [(&'a str, BorrowedFd<'a>)],
    pub(crate) timeout: Duration,
}
