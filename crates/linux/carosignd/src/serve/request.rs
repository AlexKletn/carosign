use super::{
    AuthenticatorRef, BootDeadline, IdSource, OperationContext, OwnerRef, ServeError, SubjectRef,
    TimeSource, VerificationPurpose,
};

/// What it takes to get the operation started.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Request {
    /// Subject (allowed by source of personalities).
    pub subject: SubjectRef,
    pub authenticator: AuthenticatorRef,
    /// Why.
    pub purpose: VerificationPurpose,
    pub owner: OwnerRef,
    /// How long does the operation last, ns.
    pub deadline_ns: u64,
}

impl Request {
    /// Gathers the context of the operation.
    ///
    /// ⚠️ The number comes from the source, not the service; otherwise, two orders in one
    /// millisecond would get the same number.
    pub fn context(
        &self,
        source: &dyn IdSource,
        clock: &dyn TimeSource,
    ) -> Result<OperationContext, ServeError> {
        let now = clock.now();
        // The signature does not convey the error of the clock: in case of a contradiction or
        // overflow, the period has already expired, and is not extended silently.
        let deadline = if now.boot() == clock.boot_id() {
            now.instant_ns()
                .checked_add(self.deadline_ns)
                .unwrap_or(now.instant_ns())
        } else {
            now.instant_ns()
        };
        Ok(OperationContext {
            operation_id: source.new_operation_id().map_err(|_| ServeError::NoIds)?,
            subject: self.subject,
            authenticator: self.authenticator,
            owner: self.owner,
            purpose: self.purpose,
            deadline: BootDeadline::new(now.boot(), deadline),
        })
    }
}
