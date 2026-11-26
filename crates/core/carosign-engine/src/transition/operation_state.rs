use super::CommonOperationState;
use super::MutationState;
use super::OperationBody;
use super::VerificationState;
use carosign_core::ids::MutationId;
use carosign_core::modality::OperationKind;
use carosign_core::operation::context::OperationContext;
use carosign_core::operation::mutation::EnrollRequest;
use carosign_core::operation::mutation::RemoveRequest;
use core::fmt;

/// Status of one operation.
///
/// ⚠️ Changes it (only your actor) by accepting it
/// (only)`Decision`](crate::decision::Decision).
#[derive(Clone, PartialEq, Eq)]
pub struct OperationState {
    /// Common to all species.
    pub common: CommonOperationState,
    /// This species has its own.
    pub body: OperationBody,
}

impl OperationState {
    /// The exact area that the source of authority must issue to the operations.
    ///
    /// This is only a description of the required right, not proof of its issuance.
    pub fn required_grant_scope(&self) -> carosign_core::authorization::GrantScope {
        use carosign_core::authorization::GrantScope;
        match &self.body {
            OperationBody::Verification(_) => GrantScope::Verify {
                subject: self.common.context.subject,
                authenticator: self.common.context.authenticator.id,
                purpose: self.common.context.purpose,
            },
            OperationBody::Enrollment { request, .. } => GrantScope::Enroll {
                subject: request.subject,
                authenticator: request.authenticator,
                replace: request.replace,
            },
            OperationBody::Removal { request, .. } => GrantScope::Remove {
                subject: request.subject,
                enrollment: request.enrollment,
            },
        }
    }

    /// Check-in operation.
    pub fn verification(context: OperationContext) -> Self {
        Self {
            common: CommonOperationState::start(context),
            body: OperationBody::Verification(VerificationState::default()),
        }
    }

    /// Registration operation.
    pub fn enrollment(
        context: OperationContext,
        request: EnrollRequest,
        mutation: MutationId,
    ) -> Self {
        Self {
            common: CommonOperationState::start(context),
            body: OperationBody::Enrollment {
                request,
                state: MutationState {
                    mutation,
                    plan: None,
                    registry: None,
                    reconciliation: None,
                    live_confirmation: None,
                    planned_new: None,
                },
            },
        }
    }

    /// Removal surgery.
    pub fn removal(
        context: OperationContext,
        request: RemoveRequest,
        mutation: MutationId,
    ) -> Self {
        Self {
            common: CommonOperationState::start(context),
            body: OperationBody::Removal {
                request,
                state: MutationState {
                    mutation,
                    plan: None,
                    registry: None,
                    reconciliation: None,
                    live_confirmation: None,
                    planned_new: None,
                },
            },
        }
    }

    /// Creates a registration with a full plan, verified with the request and area.
    pub fn enrollment_with_plan(
        context: OperationContext,
        request: EnrollRequest,
        mutation: MutationId,
        plan: carosign_core::operation::plan::MutationPlan,
    ) -> Result<Self, crate::decision::TransitionError> {
        use carosign_core::operation::plan::PlanTransition;
        let planned_new = match (request.replace, plan.transition()) {
            (None, PlanTransition::Create { planned_new }) => planned_new,
            (Some(requested), PlanTransition::Replace { old, planned_new }) if requested == old => {
                planned_new
            }
            _ => {
                return Err(crate::decision::TransitionError::NotSpecified {
                    what: "план не соответствует запросу регистрации",
                });
            }
        };
        if request.subject != context.subject
            || request.authenticator != context.authenticator.id
            || plan.subject() != context.subject
            || plan.authenticator() != context.authenticator.id
        {
            return Err(crate::decision::TransitionError::NotSpecified {
                what: "план регистрации вне области операции",
            });
        }
        let mut result = Self::enrollment(context, request, mutation);
        let state = result
            .body
            .mutation_mut()
            .expect("только что создана регистрация");
        state.planned_new = Some(planned_new);
        state.plan = Some(plan);
        Ok(result)
    }

    /// Creates deletion with a full plan, checked with the request and area.
    pub fn removal_with_plan(
        context: OperationContext,
        request: RemoveRequest,
        mutation: MutationId,
        plan: carosign_core::operation::plan::MutationPlan,
    ) -> Result<Self, crate::decision::TransitionError> {
        use carosign_core::operation::plan::PlanTransition;
        if request.subject != context.subject
            || request.enrollment.authenticator != context.authenticator.id
            || plan.subject() != context.subject
            || plan.authenticator() != context.authenticator.id
            || plan.transition()
                != (PlanTransition::Remove {
                    old: request.enrollment,
                })
        {
            return Err(crate::decision::TransitionError::NotSpecified {
                what: "план не соответствует запросу удаления",
            });
        }
        let mut result = Self::removal(context, request, mutation);
        result
            .body
            .mutation_mut()
            .expect("только что создано удаление")
            .plan = Some(plan);
        Ok(result)
    }

    /// Type of operation (excreted from the body).
    pub fn kind(&self) -> OperationKind {
        self.body.kind()
    }

    /// The body of change, if it is a change.
    pub fn mutation(&self) -> Option<&MutationState> {
        self.body.mutation()
    }

    /// Whether it's a change (convenient conclusion, not a second sign).
    pub fn is_mutation(&self) -> bool {
        self.body.mutation().is_some()
    }
}

impl fmt::Debug for OperationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OperationState")
            .field("body", &self.body)
            .field("common", &self.common)
            .finish()
    }
}
