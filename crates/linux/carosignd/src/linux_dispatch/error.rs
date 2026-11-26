/// Failure to execute; the caller closes the connection and does not betray success.
#[derive(Debug)]
pub(crate) enum DispatchError {
    Preparation,
    Queue(super::device_queue::QueueError),
    Provider,
    AdmissionPolicy(carosign_runtime::ports::AuthorityRefusal),
    AdmissionFlow {
        reason: &'static str,
        connection_attached: bool,
    },
    AdmissionReply {
        reason: &'static str,
        connection_attached: bool,
    },
    Channel,
    Delivery,
    Evidence,
}

impl std::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queue(error) => write!(f, "Queue({error:?})"),
            Self::Preparation => f.write_str("Preparation"),
            Self::Provider => f.write_str("Provider"),
            Self::AdmissionPolicy(reason) => write!(f, "AdmissionPolicy({reason:?})"),
            Self::AdmissionFlow {
                reason,
                connection_attached,
            } => write!(
                f,
                "AdmissionFlow(reason={reason}, connection_attached={connection_attached})"
            ),
            Self::AdmissionReply {
                reason,
                connection_attached,
            } => write!(
                f,
                "AdmissionReply(reason={reason}, connection_attached={connection_attached})"
            ),
            Self::Channel => f.write_str("Channel"),
            Self::Delivery => f.write_str("Delivery"),
            Self::Evidence => f.write_str("Evidence"),
        }
    }
}
