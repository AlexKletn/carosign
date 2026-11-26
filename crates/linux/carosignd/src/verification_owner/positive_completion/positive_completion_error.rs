use super::EffectOwnerError;

#[derive(Debug)]
pub(crate) enum PositiveCompletionError {
    Delivery(crate::operations::OwnerDeliveryFailure),
    Send(crate::operations::OwnerSendError),
    Effect(EffectOwnerError),
    Operation(crate::operations::ObservationFailure),
    InvalidStage,
}
