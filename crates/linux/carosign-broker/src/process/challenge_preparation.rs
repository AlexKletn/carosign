use carosign_protocol::carosign::{
    broker_relay::v1::Scope,
    common::v1::SubjectRef,
    provider::v1::{CommandAck, PrepareEnrollmentChallenge, ProviderFrame, StartEnrollment},
};

use super::{
    Context,
    request_validation::{checked_scope, valid_id},
};

pub(super) struct ChallengePreparation {
    command: Vec<u8>,
    mutation: Vec<u8>,
    subject: SubjectRef,
    uid: u32,
    accepted: bool,
}

impl ChallengePreparation {
    pub(super) fn begin(
        context: &Context,
        held: &Scope,
        frame: &ProviderFrame,
        challenge: &PrepareEnrollmentChallenge,
    ) -> Result<Self, ()> {
        let subject = challenge.subject.as_ref().ok_or(())?;
        let uid = frame.uid.ok_or(())?;
        if !valid_challenge_preparation(context, held, frame, challenge) {
            return Err(());
        }
        Ok(Self {
            command: frame.command_id.clone(),
            mutation: challenge.mutation_id.clone(),
            subject: subject.clone(),
            uid,
            accepted: false,
        })
    }

    pub(super) fn acknowledge(
        &mut self,
        frame: &ProviderFrame,
        ack: &CommandAck,
        held: Option<&Scope>,
    ) -> Result<(), ()> {
        if self.accepted
            || frame.command_id != self.command
            || frame.operation_id
                != held
                    .and_then(|scope| scope.owner.as_ref())
                    .ok_or(())?
                    .operation_id
            || !ack.accepted
            || ack.rejection.is_some()
        {
            return Err(());
        }
        self.accepted = true;
        Ok(())
    }

    pub(super) fn matches_continuation(
        &self,
        frame: &ProviderFrame,
        enrollment: &StartEnrollment,
    ) -> bool {
        self.accepted
            && enrollment.mutation_id == self.mutation
            && enrollment.subject.as_ref() == Some(&self.subject)
            && frame.uid == Some(self.uid)
            && frame.command_id != self.command
    }
}

fn valid_challenge_preparation(
    context: &Context,
    held: &Scope,
    frame: &ProviderFrame,
    challenge: &carosign_protocol::carosign::provider::v1::PrepareEnrollmentChallenge,
) -> bool {
    let (Some(owner), Some(authenticator), Some(subject)) = (
        held.owner.as_ref(),
        challenge.authenticator.as_ref(),
        challenge.subject.as_ref(),
    ) else {
        return false;
    };
    checked_scope(held, context)
        && held.claims.iter().all(|claim| claim.mode == 1)
        && frame.uid.is_some()
        && frame.operation_id == owner.operation_id
        && frame.worker_instance_id == context.worker
        && frame.device_epoch == context.epoch
        && valid_id(&frame.command_id)
        && valid_id(&challenge.mutation_id)
        && authenticator.authenticator_id == owner.authenticator_id
        && authenticator.device_epoch == context.epoch
        && valid_id(&subject.authority_id)
        && valid_id(&subject.subject_id)
}

#[cfg(test)]
#[path = "challenge_preparation.tests.rs"]
mod tests;
