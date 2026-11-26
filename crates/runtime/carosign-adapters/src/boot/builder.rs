use super::{BootError, BootReport};

use core::fmt;

use carosign_runtime::ports::{
    EvidencePort, MutationPort, ProviderExecutionPort, ResourcePort, TimeSource,
};

/// What does runtime come from?
pub struct Builder {
    time: Option<Box<dyn TimeSource>>,
    provider: Option<Box<dyn ProviderExecutionPort>>,
    resources: Option<Box<dyn ResourcePort>>,
    evidence: Option<Box<dyn EvidencePort>>,
    mutation: Option<Box<dyn MutationPort>>,
}

impl fmt::Debug for Builder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Builder")
            .field("time", &self.time.is_some())
            .field("provider", &self.provider.is_some())
            .field("resources", &self.resources.is_some())
            .field("evidence", &self.evidence.is_some())
            .field("mutation", &self.mutation.is_some())
            .finish()
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    /// Empty picker.
    pub fn new() -> Self {
        Self {
            time: None,
            provider: None,
            resources: None,
            evidence: None,
            mutation: None,
        }
    }

    /// Watch.
    pub fn time(mut self, time: Box<dyn TimeSource>) -> Self {
        self.time = Some(time);
        self
    }

    pub fn provider(mut self, provider: Box<dyn ProviderExecutionPort>) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn resources(mut self, resources: Box<dyn ResourcePort>) -> Self {
        self.resources = Some(resources);
        self
    }

    pub fn evidence(mut self, evidence: Box<dyn EvidencePort>) -> Self {
        self.evidence = Some(evidence);
        self
    }

    /// The magazine.
    pub fn mutation(mut self, mutation: Box<dyn MutationPort>) -> Self {
        self.mutation = Some(mutation);
        self
    }

    /// Checks and collects.
    ///
    /// ⚠️ The refusal lists the **all* missing ports: looking for them one by one is spending
    /// calls.
    pub fn compose(self) -> Result<BootReport, BootError> {
        let mut missing = Vec::new();
        if self.time.is_none() {
            missing.push("TimeSource");
        }
        if self.provider.is_none() {
            missing.push("ProviderExecutionPort");
        }
        if self.resources.is_none() {
            missing.push("ResourcePort");
        }
        if self.evidence.is_none() {
            missing.push("EvidencePort");
        }
        if self.mutation.is_none() {
            missing.push("MutationPort");
        }
        if !missing.is_empty() {
            return Err(BootError::MissingPorts { missing });
        }
        Ok(BootReport { ports: 5 })
    }
}

/// It does.
pub fn compose(builder: Builder) -> Result<BootReport, BootError> {
    builder.compose()
}
