use crate::{
    decision::TransitionError,
    transition::{OperationBody, OperationState},
};

/// The query and the context of the transaction must be in the same area.
pub(crate) fn check_scope(state: &OperationState) -> Result<(), TransitionError> {
    let context = &state.common.context;
    let valid = match &state.body {
        OperationBody::Verification(_) => true,
        OperationBody::Enrollment { request, state } => {
            request.subject == context.subject
                && request.authenticator == context.authenticator.id
                && request
                    .replace
                    .is_none_or(|old| old.authenticator == context.authenticator.id)
                && state.planned_new.is_none_or(|new| {
                    new.authenticator == context.authenticator.id
                        && request.replace.is_none_or(|old| old.id != new.id)
                })
                && state.plan.as_ref().is_none_or(|plan| {
                    use carosign_core::operation::plan::PlanTransition;
                    plan.subject() == context.subject
                        && plan.authenticator() == context.authenticator.id
                        && match (request.replace, plan.transition()) {
                            (None, PlanTransition::Create { planned_new }) => {
                                state.planned_new == Some(planned_new)
                            }
                            (Some(requested), PlanTransition::Replace { old, planned_new }) => {
                                requested == old && state.planned_new == Some(planned_new)
                            }
                            _ => false,
                        }
                })
        }
        OperationBody::Removal { request, state } => {
            request.subject == context.subject
                && request.enrollment.authenticator == context.authenticator.id
                && state.planned_new.is_none()
                && state.plan.as_ref().is_none_or(|plan| {
                    plan.subject() == context.subject
                        && plan.authenticator() == context.authenticator.id
                        && plan.transition()
                            == (carosign_core::operation::plan::PlanTransition::Remove {
                                old: request.enrollment,
                            })
                })
        }
    };
    if valid {
        Ok(())
    } else {
        Err(TransitionError::NotSpecified {
            what: "область запроса не совпадает с контекстом операции",
        })
    }
}
