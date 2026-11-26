use super::Policy;
use super::subject_of;
use carosign_core::identity::IdentityError;
use carosign_core::identity::SubjectRef;
use carosign_core::identity::SubjectSelector;
use carosign_core::ids::AuthorityId;
use carosign_runtime::ports::BoxFuture;
use carosign_runtime::ports::CallerContext;
use carosign_runtime::ports::IdentityAuthority;

/// Port of personalities on the policy file.
pub struct FileIdentity<'a> {
    policy: &'a Policy,
    authority: AuthorityId,
}

impl core::fmt::Debug for FileIdentity<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("FileIdentity(..)")
    }
}

impl<'a> FileIdentity<'a> {
    /// A source of personalities over politics.
    pub fn new(policy: &'a Policy, authority: AuthorityId) -> Self {
        Self { policy, authority }
    }
}

impl IdentityAuthority for FileIdentity<'_> {
    fn resolve(
        &self,
        caller: &CallerContext,
        selector: &SubjectSelector,
    ) -> BoxFuture<'_, Result<SubjectRef, IdentityError>> {
        // ⚠️ The uid number is taken from the context of the connection, then it's checked
        // against the policy, and the query prompt doesn't give you anything: it's just an
        // excuse to find a rule for the same caller.
        let uid = uid_of(caller);
        let result = match self.policy.unique_subject(uid) {
            Some(subject) if matches!(selector, SubjectSelector::Current) => SubjectRef {
                authority: self.authority,
                subject: subject_of(subject),
            },
            _ => return Box::pin(async { Err(IdentityError::UnknownSubject) }),
        };
        Box::pin(async move { Ok(result) })
    }
}

/// Caller uid number.
///
/// ⚠️ Taken from the context, not from the label: the byte agreement disappeared with the
/// B-06 decision.
pub(super) fn uid_of(caller: &CallerContext) -> u32 {
    caller.uid()
}
