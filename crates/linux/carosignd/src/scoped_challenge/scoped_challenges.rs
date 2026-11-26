use super::{
    BootInstant, ChallengeError, ChallengeRef, Entry, HashMap, Mutex, OperationId,
    ScopedAssertionContext,
};

/// Records remain until the end of the launch: this prohibits re-registration of the ID.
/// When the limit is exhausted, new calls are rejected rather than replaced by old ones.
pub(crate) struct ScopedChallenges {
    limit: usize,
    entries: Mutex<HashMap<ChallengeRef, Entry>>,
}

impl ScopedChallenges {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            limit,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Creates a new platform challenge for the already validated challenging context, and
    /// randomness and registration do not support that context.
    /// Repeating an existing identifier does not supersede the previous record.
    pub(crate) fn issue(
        &self,
        operation: OperationId,
        mut context: ScopedAssertionContext,
        now: BootInstant,
    ) -> Result<ScopedAssertionContext, ChallengeError> {
        for _ in 0..4 {
            let bytes =
                carosign_linux::sys::random_16().map_err(|_| ChallengeError::Unavailable)?;
            let Some(challenge) = ChallengeRef::accepted_by_recipient(bytes) else {
                continue;
            };
            context.challenge = challenge;
            match self.register(operation, context.clone(), now) {
                Ok(()) => return Ok(context),
                Err(ChallengeError::Duplicate) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(ChallengeError::Unavailable)
    }

    /// Takes context only from the caller, who is separately obliged to confirm the source of
    /// the call and the recipient.
    pub(crate) fn register(
        &self,
        operation: OperationId,
        context: ScopedAssertionContext,
        now: BootInstant,
    ) -> Result<(), ChallengeError> {
        if context.deadline.is_expired_at(now) {
            return Err(ChallengeError::Expired);
        }
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| ChallengeError::Unavailable)?;
        if entries.contains_key(&context.challenge) {
            return Err(ChallengeError::Duplicate);
        }
        if entries.len() >= self.limit {
            return Err(ChallengeError::Full);
        }
        entries.insert(
            context.challenge,
            Entry {
                operation,
                context,
                consumed: false,
            },
        );
        Ok(())
    }

    /// Checking the area and spending are carried out under one lock.
    /// The returned value is not an entry permit.
    pub(crate) fn consume(
        &self,
        operation: OperationId,
        context: &ScopedAssertionContext,
        now: BootInstant,
    ) -> Result<(), ChallengeError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| ChallengeError::Unavailable)?;
        let entry = entries
            .get_mut(&context.challenge)
            .ok_or(ChallengeError::Unknown)?;
        if entry.consumed {
            return Err(ChallengeError::Consumed);
        }
        if entry.context.deadline.is_expired_at(now) {
            entry.consumed = true;
            return Err(ChallengeError::Expired);
        }
        if entry.operation != operation || entry.context != *context {
            return Err(ChallengeError::Mismatch);
        }
        entry.consumed = true;
        Ok(())
    }
}
