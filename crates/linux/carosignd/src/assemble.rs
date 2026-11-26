//! Port and service assembly: The only place where layers meet.
//!
//! ⚠️ Rule 02: There's no state domain machine here, just wiring. Engine, actor, and ports
//! make the decisions.
//!
//! ⚠️ All the mandatory ports are mandatory: assembly fails and calls out all the missing
//! ones at once. A service that has risen without a source of authority or an inspector is a
//! service that lets everything go.

use core::fmt;

use carosign_adapters::boot::BootReport;
use carosign_runtime::ports::{
    EvidencePort, IdentityAuthority, MutationPort, PolicyAuthority, ProviderExecutionPort,
    ResourcePort, TimeSource,
};

/// Assembled service ports.
///
/// ⚠️ Ports are stored along with their types: the service gets them from the outside and
/// does not create them, so the root of the assembly remains thin, and the solutions are in
/// the ports.
pub struct Ports {
    /// Systemic source of the personality of the caller and the subject.
    pub identity: Box<dyn IdentityAuthority>,
    /// Systemic source and authority checker.
    pub policy: Box<dyn PolicyAuthority>,
    /// Watch.
    pub time: Box<dyn TimeSource>,
    pub provider: Box<dyn ProviderExecutionPort>,
    pub resources: Box<dyn ResourcePort>,
    pub evidence: Box<dyn EvidencePort>,
    /// The magazine.
    pub mutation: Box<dyn MutationPort>,
}

impl fmt::Debug for Ports {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Ports(..)")
    }
}

/// Collects service ports from ready-made parts.
///
/// Five ports of execution are being checked `adapters::Builder`The sources of identity and
/// policy are additionally mandatory in type, and the presence of an object does not in
/// itself confirm its trust and does not allow the publication of the service.
pub fn assemble(
    identity: Box<dyn IdentityAuthority>,
    policy: Box<dyn PolicyAuthority>,
    time: Box<dyn TimeSource>,
    provider: Box<dyn ProviderExecutionPort>,
    resources: Box<dyn ResourcePort>,
    evidence: Box<dyn EvidencePort>,
    mutation: Box<dyn MutationPort>,
) -> (Ports, BootReport) {
    (
        Ports {
            identity,
            policy,
            time,
            provider,
            resources,
            evidence,
            mutation,
        },
        BootReport { ports: 7 },
    )
}

#[path = "assemble.tests.rs"]
#[cfg(test)]
mod tests;
