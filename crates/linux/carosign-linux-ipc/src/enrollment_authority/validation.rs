use super::message::{PlanChange, request, response, valid_id};
use sha2::{Digest, Sha256};

pub(super) fn valid_action(action: &request::Action) -> bool {
    match action {
        request::Action::Assess(_)
        | request::Action::Revoke(_)
        | request::Action::PrepareChallenge(_) => true,
        request::Action::BindOwner(value) => {
            !value.payload.is_empty() && value.payload.len() <= 1024
        }
        request::Action::BindPlan(value) => {
            valid_id(&value.planned_enrollment)
                && value.generation != 0
                && value.plan_digest.len() == 32
                && match PlanChange::from_i32(value.change) {
                    Some(PlanChange::Create) => {
                        value.old_enrollment.is_empty() && value.old_generation == 0
                    }
                    Some(PlanChange::Replace) => {
                        valid_id(&value.old_enrollment)
                            && value.old_generation != 0
                            && value.old_enrollment != value.planned_enrollment
                    }
                    _ => false,
                }
        }
        request::Action::Validate(value) => {
            (1..=64).contains(&value.challenge.len())
                && valid_id(&value.challenge_ref)
                && (1..=60_000).contains(&value.validity_ms)
        }
        request::Action::Authorize(value)
        | request::Action::Revalidate(value)
        | request::Action::Execute(value) => {
            valid_id(&value.reference) && value.challenge_digest.len() == 32
        }
    }
}

pub(super) fn response_matches(
    action: &request::Action,
    result: &response::Result,
    grant_deadline_ns: u64,
) -> bool {
    use request::Action;
    use response::Result;
    match (action, result) {
        (_, Result::Refused(_)) => true,
        (
            Action::Assess(_)
            | Action::Revalidate(_)
            | Action::Revoke(_)
            | Action::Execute(_)
            | Action::BindOwner(_)
            | Action::BindPlan(_),
            Result::Accepted(_),
        ) => true,
        (Action::PrepareChallenge(_), Result::Challenge(value)) => {
            valid_action(&Action::Validate(value.clone()))
        }
        (Action::Validate(request), Result::Validated(result)) => {
            result.owner_binding.is_empty()
                && result.reference == request.challenge_ref
                && result.challenge_digest == Sha256::digest(&request.challenge).as_slice()
                && result.deadline_ns != 0
                && result.deadline_ns <= grant_deadline_ns
        }
        (Action::Authorize(request), Result::Authorized(result)) => {
            valid_id(&result.reference)
                && result.owner_binding.len() <= 1024
                && result.challenge_digest == request.challenge_digest
                && result.deadline_ns != 0
                && result.deadline_ns <= grant_deadline_ns
        }
        _ => false,
    }
}
