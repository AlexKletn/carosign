use super::Cancellation;

use carosign_protocol::carosign::{
    broker_relay::v1::Scope,
    provider::v1::{
        EnrollmentTransactionAction, EnrollmentTransactionRequest, EnrollmentTransactionResult,
        ProviderFrame, VerificationStatus, provider_frame::Payload,
    },
};

pub(in crate::process) struct ActiveOperation {
    command: Vec<u8>,
    mutation: Option<Vec<u8>>,
    enrollment: bool,
    cancellation: Cancellation,
}

impl ActiveOperation {
    pub(in crate::process) fn verification(command: &[u8]) -> Self {
        Self {
            command: command.to_vec(),
            mutation: None,
            enrollment: false,
            cancellation: Cancellation::None,
        }
    }

    pub(in crate::process) fn enrollment(command: &[u8], mutation: &[u8]) -> Self {
        Self {
            command: command.to_vec(),
            mutation: Some(mutation.to_vec()),
            enrollment: true,
            cancellation: Cancellation::None,
        }
    }

    pub(in crate::process) fn removal(command: &[u8], mutation: &[u8]) -> Self {
        Self {
            command: command.to_vec(),
            mutation: Some(mutation.to_vec()),
            enrollment: false,
            cancellation: Cancellation::None,
        }
    }

    pub(in crate::process) fn is_cancelled(&self) -> bool {
        !matches!(self.cancellation, Cancellation::None)
    }

    pub(in crate::process) fn cancel(&mut self, cancel_command: &[u8]) {
        self.cancellation = Cancellation::Requested(cancel_command.to_vec());
    }

    pub(in crate::process) fn begin_rollback(
        &mut self,
        scope: Option<&Scope>,
        frame: &ProviderFrame,
        request: &EnrollmentTransactionRequest,
    ) -> Result<(), ()> {
        if !self.enrollment || self.is_cancelled() {
            return Err(());
        }
        let owner = scope.and_then(|scope| scope.owner.as_ref()).ok_or(())?;
        if request.action != EnrollmentTransactionAction::EnrollmentTransactionRollback as i32
            || self.mutation.as_ref() != Some(&request.mutation_id)
            || self.command != request.enrollment_command_id
            || frame.operation_id != owner.operation_id
        {
            return Err(());
        }
        self.cancellation = Cancellation::RollingBack(frame.command_id.clone());
        Ok(())
    }

    pub(in crate::process) fn finish_rollback(
        &mut self,
        scope: Option<&Scope>,
        frame: &ProviderFrame,
        result: &EnrollmentTransactionResult,
    ) -> Result<(), ()> {
        if !matches!(&self.cancellation, Cancellation::RollingBack(command) if command == &frame.command_id)
        {
            return Err(());
        }
        let owner = scope.and_then(|scope| scope.owner.as_ref()).ok_or(())?;
        if frame.operation_id != owner.operation_id
            || self.mutation.as_ref() != Some(&result.mutation_id)
            || self.command != result.enrollment_command_id
        {
            return Err(());
        }
        self.cancellation = Cancellation::RollbackResultReceived;
        Ok(())
    }

    pub(in crate::process) fn check_event(
        &self,
        scope: Option<&Scope>,
        frame: &ProviderFrame,
        payload: &Payload,
    ) -> Result<(), ()> {
        let owner = scope.and_then(|scope| scope.owner.as_ref()).ok_or(())?;
        if frame.operation_id != owner.operation_id {
            return Err(());
        }
        if matches!(payload, Payload::CancelReceipt(_)) {
            if !matches!(&self.cancellation, Cancellation::Requested(command) if command == &frame.command_id)
            {
                return Err(());
            }
        } else if self.command != frame.command_id {
            return Err(());
        }
        if let Payload::MutationOutcome(outcome) = payload
            && self.mutation.as_ref() != Some(&outcome.mutation_id)
        {
            return Err(());
        }
        if let Payload::VerificationOutcome(outcome) = payload
            && (self.mutation.is_some()
                || (self.is_cancelled()
                    && outcome.status != VerificationStatus::VerifyCancelled as i32))
        {
            return Err(());
        }
        Ok(())
    }
}
