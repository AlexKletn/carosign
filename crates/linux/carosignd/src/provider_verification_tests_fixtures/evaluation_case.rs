#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EvaluationCase {
    Normal,
    ScopedAssertion,
    WrongAssessment,
    Revoked,
    RevokedDuring,
    OwnerRevokedDuring,
    RevokedAfterCleanup,
    ExpiredAfterCleanup,
    Interrupted,
}
