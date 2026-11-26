use super::{Action, Policy, file_identity::uid_of, subject_encoding::local_subject};
use carosign_core::authorization::GrantScope;
use carosign_core::authorization::ValidatedGrant;
use carosign_core::ids::AuthorityId;
use carosign_core::ids::BootId;
use carosign_core::ids::GrantId;
use carosign_runtime::ports::AuthorityRefusal;
use carosign_runtime::ports::BoxFuture;
use carosign_runtime::ports::CallerContext;
use carosign_runtime::ports::PolicyAuthority;
use carosign_runtime::ports::TimeSource;

/// Port of authority for a snapshot of disassembled policies.
///
/// Borrowed `Policy` It does not change during the lifetime of the port, so it does not see a
/// file update or review from an external source, and is not in itself suitable as a system
/// source of production authority.
pub struct FilePolicy<'a> {
    policy: &'a Policy,
    authority: AuthorityId,
    boot: BootId,
    clock: Option<&'a dyn TimeSource>,
}

impl core::fmt::Debug for FilePolicy<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("FilePolicy(..)")
    }
}

impl<'a> FilePolicy<'a> {
    /// The source of authority over politics.
    pub fn new(policy: &'a Policy, authority: AuthorityId, boot: BootId) -> Self {
        Self {
            policy,
            authority,
            boot,
            clock: None,
        }
    }

    /// The source of the policy with the monotonous hours of the current load.
    pub fn with_clock(
        policy: &'a Policy,
        authority: AuthorityId,
        clock: &'a dyn TimeSource,
    ) -> Self {
        Self {
            policy,
            authority,
            boot: clock.boot_id(),
            clock: Some(clock),
        }
    }
}

impl PolicyAuthority for FilePolicy<'_> {
    fn authorize(
        &self,
        caller: &CallerContext,
        scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        let uid = uid_of(caller);
        let subject_ref = scope.subject();
        let Some(subject_ref) = subject_ref else {
            // Observe/Cancel must be authorized to operate on its registry with confidence;
            // ISP management is also outside this policy.
            return Box::pin(async { Err(AuthorityRefusal::Denied) });
        };
        let Some(subject) = local_subject(subject_ref, self.authority) else {
            return Box::pin(async { Err(AuthorityRefusal::Denied) });
        };
        let action = action_of(&scope);
        if !self.policy.permits(uid, subject, action) {
            return Box::pin(async { Err(AuthorityRefusal::Denied) });
        }
        let Some(clock) = self.clock else {
            return Box::pin(async { Err(AuthorityRefusal::Unavailable) });
        };
        let now = clock.now();
        if now.boot() != self.boot || clock.boot_id() != self.boot {
            return Box::pin(async { Err(AuthorityRefusal::Unavailable) });
        }
        let Some(lifetime) = self.policy.lifetime_s(uid, subject) else {
            return Box::pin(async { Err(AuthorityRefusal::Denied) });
        };
        let Some(deadline) = lifetime
            .checked_mul(1_000_000_000)
            .and_then(|duration| now.instant_ns().checked_add(duration))
        else {
            return Box::pin(async { Err(AuthorityRefusal::Unavailable) });
        };
        if deadline <= now.instant_ns() {
            return Box::pin(async { Err(AuthorityRefusal::Expired) });
        }
        let grant = ValidatedGrant::issue_for_port(
            /**/
            GrantId::from_bytes(uid_bytes(uid)).expect("ненулевой"),
            self.authority,
            scope,
            self.boot,
            deadline,
        );
        Box::pin(async move { Ok(grant) })
    }

    fn revalidate(
        &self,
        grant: &ValidatedGrant,
        _boundary: carosign_core::authorization::AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        let Some(clock) = self.clock else {
            return Box::pin(async { Err(AuthorityRefusal::Unavailable) });
        };
        let now = clock.now();
        if now.boot() != self.boot || clock.boot_id() != self.boot || grant.boot() != self.boot {
            return Box::pin(async { Err(AuthorityRefusal::Expired) });
        }
        if now.instant_ns() >= grant.deadline_ns() {
            return Box::pin(async { Err(AuthorityRefusal::Expired) });
        }
        // ⚠️ It's just a recheck of a retained image, and it doesn't see a file change, and
        // it doesn't replace independent review.
        let scope = grant.scope().clone();
        let valid = match scope.subject() {
            Some(subject_ref) => {
                local_subject(subject_ref, self.authority).is_some_and(|subject| {
                    grant.issuer() == self.authority
                        && grant.boot() == self.boot
                        && *grant.id().as_bytes() == uid_bytes(grant_uid(grant))
                        && self
                            .policy
                            .permits(grant_uid(grant), subject, action_of(&scope))
                })
            }
            None => false,
        };
        Box::pin(async move {
            if valid {
                Ok(())
            } else {
                Err(AuthorityRefusal::Revoked)
            }
        })
    }
}

fn action_of(scope: &GrantScope) -> Action {
    match scope {
        GrantScope::Verify { .. } => Action::Verify,
        GrantScope::Enroll { .. } => Action::Enroll,
        GrantScope::Remove { .. } => Action::Remove,
        GrantScope::Observe(_) => Action::Observe,
        GrantScope::Cancel(_) => Action::Cancel,
        GrantScope::ManageProvider => Action::Enroll,
    }
}

/// The number of uids is 16 bytes suitable for the resolution number.
///
/// ⚠️ Zero is not allowed as a number, so the first byte is a label, and the uid lies in the
/// last four.
fn uid_bytes(uid: u32) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[0] = 1;
    bytes[12..].copy_from_slice(&uid.to_be_bytes());
    bytes
}

/// Whose uid's in.
fn grant_uid(grant: &ValidatedGrant) -> u32 {
    let id = grant.id();
    let bytes = id.as_bytes();
    u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]])
}
