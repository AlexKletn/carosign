use super::AcceptOutcome;

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum CountedAcceptError {
    Accept(AcceptOutcome),
    AccountingUnavailable,
    RandomUnavailable,
}
