use super::{SessionError, SessionPhase};

use carosign_protocol::carosign::client::v1 as client;
use carosign_protocol::carosign::client::v1::client_frame::Payload;
use carosign_protocol::limits;
use carosign_protocol::version::{self, Negotiated, VersionMismatch};
use prost::Message;

use crate::codec::{self, ClientRequest};
use crate::credentials::PeerCredentials;
use crate::frame;

/// Limits to welcome, ms.
pub const HELLO_TIMEOUT_MS: u64 = 5_000;

/// Limit of connection operations (Document 13).
pub const OPS_PER_CONNECTION: usize = 4;

/// One client session.
#[derive(Debug)]
pub struct ClientSession {
    capture_feedback_v1: bool,
    peer: PeerCredentials,
    phase: SessionPhase,
    negotiated: Option<Negotiated>,
    hello_request_id: Option<Vec<u8>>,
    connection: [u8; 16],
}

impl ClientSession {
    /// New session: Creeds removed, no welcome yet.
    pub fn new(peer: PeerCredentials, connection: [u8; 16]) -> Self {
        Self {
            capture_feedback_v1: false,
            peer,
            phase: SessionPhase::AwaitingHello,
            negotiated: None,
            hello_request_id: None,
            connection,
        }
    }

    /// Who's here?
    pub fn peer(&self) -> PeerCredentials {
        self.peer
    }

    /// Where's the session?
    pub fn phase(&self) -> SessionPhase {
        self.phase
    }

    /// Agreed version.
    pub fn negotiated(&self) -> Option<Negotiated> {
        self.negotiated
    }

    pub fn capture_feedback_v1(&self) -> bool {
        self.capture_feedback_v1
    }

    /// Connection identifier (not authority).
    pub fn connection(&self) -> [u8; 16] {
        self.connection
    }

    /// He's taking a shot.
    ///
    /// ⚠️ The first message must be a greeting. The rest is a refusal, even if it asks for
    /// "just a list."
    pub fn accept(&mut self, bytes: &[u8]) -> Result<ClientRequest, SessionError> {
        if self.phase == SessionPhase::Closed {
            return Err(SessionError::Closed);
        }
        frame::check(bytes, false).map_err(SessionError::Frame)?;

        let (raw, request) = codec::decode(bytes).map_err(SessionError::Request)?;

        match (self.phase, &request) {
            (
                SessionPhase::AwaitingHello,
                ClientRequest::Hello {
                    major,
                    min_minor,
                    max_minor,
                },
            ) => {
                let negotiated = version::negotiate(*major, *min_minor, *max_minor)
                    .map_err(|_: VersionMismatch| SessionError::VersionMismatch)?;
                self.capture_feedback_v1 =
                    matches!(raw.payload, Some(Payload::Hello(hello)) if hello.capture_feedback_v1);
                self.negotiated = Some(negotiated);
                self.hello_request_id = Some(raw.request_id.clone());
                self.phase = SessionPhase::Ready;
                Ok(request)
            }
            (SessionPhase::AwaitingHello, _) => Err(SessionError::HelloExpected),
            (SessionPhase::Ready, ClientRequest::Hello { .. }) => Err(SessionError::AlreadyHelloed),
            (SessionPhase::Ready, _) => {
                // ⚠️ Version verification in each frame: the client could "forget" minor.
                let negotiated = self.negotiated.ok_or(SessionError::VersionMismatch)?;
                if raw.major != negotiated.major || raw.minor != negotiated.minor {
                    return Err(SessionError::VersionMismatch);
                }
                Ok(request)
            }
            (SessionPhase::Closed, _) => Err(SessionError::Closed),
        }
    }

    /// Answer to greetings.
    pub fn hello_ack(&self) -> Option<Vec<u8>> {
        let negotiated = self.negotiated?;
        let request_id = self.hello_request_id.as_deref()?;
        let frame = codec::hello_ack(
            request_id,
            negotiated.major,
            negotiated.minor,
            self.connection,
            self.capture_feedback_v1,
        );
        Some(frame.encode_to_vec())
    }

    /// Closing the session.
    pub fn close(&mut self) {
        self.phase = SessionPhase::Closed;
    }
}

/// Assistant: Collects a frame of greetings for inspections and customers.
pub fn hello_frame(request_id: [u8; 16], min_minor: u32, max_minor: u32) -> Vec<u8> {
    client::ClientFrame {
        major: limits::MAJOR,
        minor: limits::MINOR_BASELINE,
        request_id: request_id.to_vec(),
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(Payload::Hello(client::Hello {
            capture_feedback_v1: false,
            major: limits::MAJOR,
            min_minor,
            max_minor,
        })),
    }
    .encode_to_vec()
}

/// Checking the client frame is for those who collect it themselves.
pub use carosign_protocol::validation::check_client_frame as check;
