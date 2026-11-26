use super::ObservationFailure;

/// Why did the inspection report not end?
#[derive(Debug)]
pub enum VerificationPreparationFailure {
    /// The operation or its context does not permit preparation.
    Operation(ObservationFailure),
    /// The source of authority refused to re-check.
    Authority(carosign_runtime::ports::AuthorityRefusal),
    /// The permit changed during the re-check.
    GrantChanged,
}
