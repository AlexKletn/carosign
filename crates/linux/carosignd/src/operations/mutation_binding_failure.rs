use super::ObservationFailure;

/// Refusal of binding altering shipment to sustainable armament.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum MutationBindingFailure {
    Operation(ObservationFailure),
    Authority(carosign_runtime::ports::AuthorityRefusal),
    Resource(carosign_runtime::ports::ResourceDenial),
}
