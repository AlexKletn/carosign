use carosign_runtime::ports::AuthorityRefusal;

use super::{ClientRequest, Connection, Ended, common};

/// The performer receives only the request and the connection location already verified by
/// the session, and the sources of trust and ownership of the operation determine the
/// composition of the service.
#[allow(dead_code)]
pub(crate) trait VerificationDispatcher {
    fn cancel_operation(
        &self,
        _connection: &Connection,
        _operation: carosign_core::ids::OperationId,
    ) -> Result<carosign_protocol::carosign::client::v1::CancelReceiptKind, AuthorityRefusal> {
        Err(AuthorityRefusal::Unavailable)
    }

    fn observe_operation(
        &self,
        _connection: &Connection,
        _operation: carosign_core::ids::OperationId,
        _after_sequence: u64,
        _request_id: &[u8],
    ) -> Result<(), Ended> {
        Err(Ended::Refused("observation unavailable".into()))
    }

    fn operation_status(
        &self,
        _connection: &Connection,
        _operation: carosign_core::ids::OperationId,
    ) -> Result<carosign_protocol::carosign::client::v1::OperationSnapshot, AuthorityRefusal> {
        Err(AuthorityRefusal::Unavailable)
    }

    fn inventory_for(
        &self,
        _connection: &Connection,
    ) -> Result<carosign_protocol::carosign::client::v1::Authenticators, AuthorityRefusal> {
        Err(AuthorityRefusal::Unavailable)
    }
    /// The list is returned only after the permission of the subject and the policy.
    fn enrollments<'a>(
        &'a self,
        _connection: &'a Connection,
        _request: &'a ClientRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<common::EnrollmentSummary>, ()>> + 'a>,
    > {
        Box::pin(async { Err(()) })
    }
    fn rename_enrollment<'a>(
        &'a self,
        _connection: &'a Connection,
        _request: &'a carosign_protocol::carosign::client::v1::RenameEnrollment,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<
        carosign_protocol::carosign::client::v1::EnrollmentRenamed,
        carosign_protocol::carosign::common::v1::ErrorCode,
    >> + 'a>> {
        Box::pin(async { Err(common::ErrorCode::ErrorUnsupported) })
    }

    fn dispatch<'a>(
        &'a self,
        connection: &'a Connection,
        handle: crate::operations::ConnectionHandle,
        request: &'a ClientRequest,
        request_id: &'a [u8],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Ended>> + 'a>>;
}
