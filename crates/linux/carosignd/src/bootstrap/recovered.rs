use super::{BlockedMutationScope, BootstrapError, MutationId, SchemaStatus};

/// Which I found out before it was published.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovered {
    /// Is the version of the scheme and the status of migration confirmed?
    pub schema: SchemaStatus,
    /// Areas that can not be touched until the reconciliation.
    pub blocked: Vec<MutationId>,
    /// Areas checked together with the journal in one picture.
    pub blocked_scopes: Vec<BlockedMutationScope>,
    /// Records from which no one can judge.
    pub needs_decision: Vec<MutationId>,
}

impl Recovered {
    /// Creates a fence for all areas of the verified image.
    pub fn coordinator(
        &self,
    ) -> Result<carosign_runtime::recovery::blocked_scope::RecoveryCoordinator, BootstrapError>
    {
        use carosign_runtime::recovery::blocked_scope::{RecoveryCoordinator, RecoveryScope};
        let ids: std::collections::BTreeSet<_> = self.blocked.iter().copied().collect();
        let scopes: std::collections::BTreeSet<_> = self
            .blocked_scopes
            .iter()
            .map(|entry| entry.mutation)
            .collect();
        if !matches!(self.schema, SchemaStatus::V3 | SchemaStatus::V4)
            || ids.len() != self.blocked.len()
            || scopes.len() != self.blocked_scopes.len()
            || ids != scopes
        {
            return Err(BootstrapError::Scan(
                "области восстановления не подтверждены".into(),
            ));
        }
        let mut coordinator = RecoveryCoordinator::new();
        for entry in &self.blocked_scopes {
            coordinator
                .block(
                    RecoveryScope {
                        subject: entry.subject,
                        authenticator: entry.authenticator,
                    },
                    entry.mutation,
                )
                .map_err(|error| BootstrapError::Scan(format!("ограждение области: {error:?}")))?;
        }
        Ok(coordinator)
    }
}
