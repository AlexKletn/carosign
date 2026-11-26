use super::SignatureRefusal;

/// Error of running a separate verification process or its report.
#[derive(Debug)]
pub(in crate::discovery) enum VerificationError {
    Io(std::io::Error),
    Timeout,
    OutputLimit,
    Signature(SignatureRefusal),
}
