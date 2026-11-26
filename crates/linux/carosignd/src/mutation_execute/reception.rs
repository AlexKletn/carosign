use super::ObservedMutation;

pub(crate) enum MutationReception {
    Observed(ObservedMutation),
    RolledBack(Option<carosign_protocol::carosign::common::v1::ErrorCode>),
}
