use super::{BootId, OperationId, client, common};

/// What to say to the client.
#[derive(Clone, PartialEq, Debug)]
pub enum Reply {
    /// Operation accepted: number and deadline.
    Accepted {
        /// Number.
        operation: OperationId,
        /// Deadline (loading number and moment).
        boot: BootId,
        /// Moment, ns.
        instant_ns: u64,
    },
    /// Refusal.
    Refused(common::Error),
}

impl Reply {
    /// Response frame.
    ///
    /// ⚠️ `request_id` It echoes back to the client: without it, the answer cannot be linked
    /// to the request.
    pub fn frame(&self, request_id: &[u8]) -> client::ClientFrame {
        match self {
            Self::Accepted {
                operation,
                boot,
                instant_ns,
            } => client::ClientFrame {
                major: carosign_protocol::limits::MAJOR,
                minor: carosign_protocol::limits::MINOR_MAX,
                request_id: request_id.to_vec(),
                operation_id: operation.as_bytes().to_vec(),
                event_sequence: 0,
                payload: Some(client::client_frame::Payload::OperationAccepted(
                    client::OperationAccepted {
                        operation_id: operation.as_bytes().to_vec(),
                        first_event_sequence: 1,
                        deadline: Some(common::BootDeadline {
                            boot_id: boot.as_bytes().to_vec(),
                            boottime_ns: *instant_ns,
                        }),
                    },
                )),
            },
            Self::Refused(error) => client::ClientFrame {
                major: carosign_protocol::limits::MAJOR,
                minor: carosign_protocol::limits::MINOR_MAX,
                request_id: request_id.to_vec(),
                operation_id: Vec::new(),
                event_sequence: 0,
                payload: Some(client::client_frame::Payload::ProtocolError(
                    client::ProtocolError {
                        error: Some(error.clone()),
                    },
                )),
            },
        }
    }
}
