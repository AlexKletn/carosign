//! One-time delivery of a stable change result to the original connection.
//!
//! `Completed` This is permissible only after a confirmed cleaning by the performer; reading
//! the record does not replace checking the live recipient and his/her authority.

use carosign_core::enrollment::EnrollmentRef;
use carosign_core::ids::{MutationId, OperationId};
use carosign_persistence::model::{MutationPhase, MutationRecord, MutationTerminalOutcome};
use carosign_persistence::store::MetadataStore;
use carosign_protocol::carosign::{client::v1 as client, common::v1 as common};
use carosign_runtime::ports::CallerContext;

use crate::mutation_dispatch::PreparedMutation;

fn match_record(
    record: &MutationRecord,
    subject: carosign_core::identity::SubjectRef,
    authenticator: carosign_core::ids::AuthenticatorId,
    expected_plan: &carosign_persistence::mutation_plan::MutationPlan,
) -> Result<(MutationTerminalOutcome, Option<EnrollmentRef>), DeliveryError> {
    if record.phase != MutationPhase::Completed
        || record.subject != subject
        || record.authenticator != authenticator
        || record.plan.as_ref() != Some(expected_plan)
    {
        return Err(DeliveryError::Unavailable);
    }
    let outcome = record.terminal_outcome.ok_or(DeliveryError::Unavailable)?;
    let enrollment = if outcome == MutationTerminalOutcome::Committed {
        use carosign_persistence::mutation_plan::Transition;
        let plan = record.plan.as_ref().ok_or(DeliveryError::Unavailable)?;
        match plan.transition {
            Transition::EnrollNew | Transition::EnrollReplace => Some(EnrollmentRef {
                id: plan.new_enrollment.ok_or(DeliveryError::Unavailable)?,
                authenticator: record.authenticator,
                generation: carosign_core::enrollment::EnrollmentGeneration::new(
                    plan.new_generation,
                )
                .map_err(|_| DeliveryError::Unavailable)?,
            }),
            Transition::Remove => Some(plan.old_target.ok_or(DeliveryError::Unavailable)?),
        }
    } else {
        None
    };
    Ok((outcome, enrollment))
}

#[path = "mutation_delivery.tests.rs"]
#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests;

fn terminal(
    operation: OperationId,
    mutation: MutationId,
    sequence: u64,
    outcome: MutationTerminalOutcome,
    enrollment: Option<EnrollmentRef>,
) -> client::ClientFrame {
    let result = match outcome {
        MutationTerminalOutcome::Committed => client::MutationResult::MutationCommitted,
        MutationTerminalOutcome::NoChange => client::MutationResult::MutationNoChange,
    };
    let terminal = client::TerminalResult {
        kind: Some(client::terminal_result::Kind::Mutation(
            client::MutationTerminal {
                result: result as i32,
                enrollment: enrollment.map(|value| common::EnrollmentRef {
                    enrollment_id: value.id.as_bytes().to_vec(),
                    authenticator_id: value.authenticator.as_bytes().to_vec(),
                    generation: value.generation.value(),
                }),
                mutation_id: mutation.as_bytes().to_vec(),
                error: None,
            },
        )),
    };
    client::ClientFrame {
        major: carosign_protocol::limits::MAJOR,
        minor: carosign_protocol::limits::MINOR_MAX,
        request_id: Vec::new(),
        operation_id: operation.as_bytes().to_vec(),
        event_sequence: sequence,
        payload: Some(client::client_frame::Payload::OperationEvent(
            client::OperationEvent {
                capture_state: None,
                feedback: None,
                operation_id: operation.as_bytes().to_vec(),
                sequence,
                kind: client::EventKind::EventTerminal as i32,
                phase: client::OperationPhase::OpTerminal as i32,
                percent: 0,
                gap_after_sequence: 0,
                snapshot: None,
                terminal: Some(terminal),
            },
        )),
    }
}

mod delivery_error;
pub(crate) use delivery_error::DeliveryError;
mod mutation_delivery;
pub(crate) use mutation_delivery::MutationDelivery;
