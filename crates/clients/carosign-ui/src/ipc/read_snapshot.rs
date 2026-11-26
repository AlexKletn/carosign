use carosign_linux_ipc::client::transport::{Transport, TransportError};
use carosign_protocol::carosign::client::v1::{
    Authenticators, GetHealth, Health, ListAuthenticators, client_frame::Payload,
};
use carosign_protocol::carosign::{client::v1::ProtocolError, common::v1::ErrorCode};

use crate::domain::{QueryError, Snapshot};

use super::read_enrollments::read_enrollments;

pub(super) const CONTROL_SOCKET: &str = "/run/carosignd/control.sock";

pub(super) fn read_snapshot() -> Snapshot {
    match Transport::connect_root(CONTROL_SOCKET) {
        Ok(transport) => {
            let health = read_health(&transport);
            let inventory = read_inventory(&transport);
            let enrollments = inventory
                .as_ref()
                .map_err(|error| *error)
                .and_then(|inventory| read_enrollments(&transport, inventory));
            Snapshot {
                health,
                inventory,
                enrollments,
            }
        }
        Err(error) => Snapshot {
            health: Err(error.into()),
            inventory: Err(error.into()),
            enrollments: Err(error.into()),
        },
    }
}

fn read_health(transport: &Transport) -> Result<Health, QueryError> {
    match transport.request(Payload::GetHealth(GetHealth {}))?.payload {
        Some(Payload::Health(health)) => Ok(health),
        Some(Payload::ProtocolError(error)) => Err(error.into()),
        _ => Err(QueryError::Unexpected),
    }
}

fn read_inventory(transport: &Transport) -> Result<Authenticators, QueryError> {
    match transport
        .request(Payload::ListAuthenticators(ListAuthenticators {}))?
        .payload
    {
        Some(Payload::Authenticators(inventory)) => Ok(inventory),
        Some(Payload::ProtocolError(error)) => Err(error.into()),
        _ => Err(QueryError::Unexpected),
    }
}

impl From<TransportError> for QueryError {
    fn from(error: TransportError) -> Self {
        match error {
            TransportError::Unavailable => Self::Unavailable,
            TransportError::Unexpected => Self::Unexpected,
        }
    }
}

impl From<ProtocolError> for QueryError {
    fn from(error: ProtocolError) -> Self {
        match error.error.map(|error| error.code()) {
            Some(ErrorCode::ErrorUnauthorized) => Self::Unauthorized,
            Some(ErrorCode::ErrorUnavailable) => Self::Unavailable,
            _ => Self::Unexpected,
        }
    }
}
