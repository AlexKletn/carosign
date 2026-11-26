use std::{future::Future, pin::Pin};

use carosign_core::ids::OperationId;
use carosign_linux_ipc::codec::ClientRequest;
use carosign_protocol::carosign::{
    client::v1::{Authenticators, OperationSnapshot},
    common::v1::EnrollmentSummary,
};
use carosign_runtime::ports::AuthorityRefusal;

use crate::{
    operations::ConnectionHandle,
    server::Connection,
    talk::{Ended, VerificationDispatcher},
};

use super::operation_status::OperationStatus;

pub(super) struct ObservedDispatcher<'a> {
    pub(super) inner: &'a dyn VerificationDispatcher,
    pub(super) operations: OperationStatus<'a>,
}

impl VerificationDispatcher for ObservedDispatcher<'_> {
    fn cancel_operation(
        &self,
        connection: &Connection,
        operation: OperationId,
    ) -> Result<carosign_protocol::carosign::client::v1::CancelReceiptKind, AuthorityRefusal> {
        let caller = crate::cycle::caller_from_session(&connection.session)
            .ok_or(AuthorityRefusal::Denied)?;
        super::cancellation_request::cancel(&self.operations, &caller, operation)
    }

    fn observe_operation(
        &self,
        connection: &Connection,
        operation: OperationId,
        after_sequence: u64,
        request_id: &[u8],
    ) -> Result<(), Ended> {
        super::observation_stream::observe(
            &self.operations,
            connection,
            operation,
            after_sequence,
            request_id,
        )
    }

    fn operation_status(
        &self,
        connection: &Connection,
        operation: OperationId,
    ) -> Result<OperationSnapshot, AuthorityRefusal> {
        let caller = crate::cycle::caller_from_session(&connection.session)
            .ok_or(AuthorityRefusal::Denied)?;
        self.operations.read(caller.uid(), operation)
    }

    fn inventory_for(&self, connection: &Connection) -> Result<Authenticators, AuthorityRefusal> {
        self.inner.inventory_for(connection)
    }

    fn enrollments<'a>(
        &'a self,
        connection: &'a Connection,
        request: &'a ClientRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<EnrollmentSummary>, ()>> + 'a>> {
        self.inner.enrollments(connection, request)
    }

    fn rename_enrollment<'a>(
        &'a self,
        connection: &'a Connection,
        request: &'a carosign_protocol::carosign::client::v1::RenameEnrollment,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<
        carosign_protocol::carosign::client::v1::EnrollmentRenamed,
        carosign_protocol::carosign::common::v1::ErrorCode,
    >> + 'a>> {
        self.inner.rename_enrollment(connection, request)
    }

    fn dispatch<'a>(
        &'a self,
        connection: &'a Connection,
        handle: ConnectionHandle,
        request: &'a ClientRequest,
        request_id: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), Ended>> + 'a>> {
        self.inner.dispatch(connection, handle, request, request_id)
    }
}
