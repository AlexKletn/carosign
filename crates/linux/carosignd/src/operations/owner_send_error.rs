use super::OwnerDeliveryFailure;

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum OwnerSendError {
    Delivery(OwnerDeliveryFailure),
    WrongConnection,
    Finished,
    Socket(i32),
}
