use super::EnrollmentAuthorization;
use super::MutationChange;

/// What we're sending.
///
/// ⚠️ Checking is required to carry a set of authorized registrations (Document 29, D-03).
///
/// ⚠️ Various sizes on purpose: registration sets are larger than numbers. `Box` For the sake
/// of the same size, it would mean chasing excess memory at every step.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CommandPlan {
    /// Verification of a specific set of registrations.
    Verify {
        /// Who's allowed to see.
        permitted: crate::operation::bindings::PermittedSet,
    },
    /// Modification: **Requires** Confirmed permission to ship.
    Mutate {
        /// Change number already on the disk.
        mutation: carosign_core::ids::MutationId,
        /// Command number already in line.
        command: carosign_core::ids::CommandId,
        /// As a provider, you are convinced of the right to register.
        authorization: EnrollmentAuthorization,
        /// What exactly do we change: registration or deletion?
        ///
        /// ⚠️ Deletion must be bundled (Document 29, D-07): a single reference cannot
        /// distinguish "this record" from "this record in the vault."
        change: MutationChange,
    },
    /// Safe condition question.
    Query,
    /// Prepare a call for registration (document 31).
    ///
    /// ⚠️ Allowed **Only** for the provider who has declared the regime `CHALLENGED`, and
    /// only if his behavior at this step is confirmed does not change the stable state.
    PrepareEnrollmentChallenge {
        mutation: carosign_core::ids::MutationId,
    },
}
