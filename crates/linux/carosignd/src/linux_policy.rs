//! The System Source of Linux Personality and Policy.
//!
//! The path is verified as a secure root-owned image, and the system installation, root,
//! kernel, and OS services are the trusted boundary of this composition.

use std::path::Path;
use std::sync::Arc;

use carosign_core::authorization::{AuthorizationBoundary, GrantScope, ValidatedGrant};
use carosign_core::identity::{IdentityError, SubjectRef, SubjectSelector};
use carosign_core::ids::AuthorityId;
use carosign_runtime::authority::verification::VerificationIntent;
use carosign_runtime::ports::{
    AuthorityRefusal, BoxFuture, CallerContext, IdentityAuthority, PolicyAuthority, TimeSource,
};

use crate::policy_authority::SnapshotAuthority;

/// The owner of the picture is used simultaneously for the person and authority.
pub struct LinuxPolicy(SnapshotAuthority);

impl LinuxPolicy {
    /// Returns the verified entity's UID only from its own Linux authority area. Client UID
    /// and broker process UID are not suitable.
    pub(crate) fn uid_of_subject(&self, subject: SubjectRef) -> Result<u32, IdentityError> {
        if subject.authority.as_bytes() != b"linux-local-v001" {
            return Err(IdentityError::UnknownSubject);
        }
        let bytes = subject.subject.as_bytes();
        if bytes[..8] != [1, 0, 0, 0, 0, 0, 0, 0] {
            return Err(IdentityError::UnknownSubject);
        }
        let value = u64::from_be_bytes(bytes[8..].try_into().expect("длина SubjectId"));
        u32::try_from(value).map_err(|_| IdentityError::UnknownSubject)
    }
    pub(crate) fn is_current(&self) -> bool {
        self.0.is_current()
    }
    pub(crate) fn read_inventory<T>(
        &self,
        connection: &crate::server::Connection,
        read: impl FnOnce() -> Result<T, ()>,
    ) -> Result<T, AuthorityRefusal> {
        let caller = crate::cycle::caller_from_session(&connection.session)
            .ok_or(AuthorityRefusal::Denied)?;
        self.0.authorize_inventory(caller.uid())?;
        let inventory = read().map_err(|()| AuthorityRefusal::Unavailable)?;
        self.0.authorize_inventory(caller.uid())?;
        Ok(inventory)
    }

    pub(crate) fn authorize_observation(
        &self,
        uid: u32,
        subject: Option<SubjectRef>,
    ) -> Result<(), AuthorityRefusal> {
        self.authorize_operation_action(uid, subject, carosign_adapters::policy::Action::Observe)
    }

    pub(crate) fn authorize_cancel(
        &self,
        uid: u32,
        subject: Option<SubjectRef>,
        owner: bool,
    ) -> Result<(), AuthorityRefusal> {
        if owner {
            return self
                .is_current()
                .then_some(())
                .ok_or(AuthorityRefusal::Revoked);
        }
        self.authorize_operation_action(uid, subject, carosign_adapters::policy::Action::Cancel)
    }

    fn authorize_operation_action(
        &self,
        uid: u32,
        subject: Option<SubjectRef>,
        action: carosign_adapters::policy::Action,
    ) -> Result<(), AuthorityRefusal> {
        let subject = subject
            .map(|subject| {
                self.uid_of_subject(subject)
                    .map_err(|_| AuthorityRefusal::Denied)
            })
            .transpose()?;
        self.0.authorize_operation_action(uid, subject, action)
    }

    /// The identity of the subject; the right of registration or deletion is verified
    /// separately.
    pub(crate) fn resolve_mutation_subject(
        &self,
        caller: &CallerContext,
        selector: &carosign_protocol::carosign::common::v1::SubjectSelector,
    ) -> Result<SubjectRef, IdentityError> {
        self.0.resolve_mutation_subject(caller, selector)
    }
    #[cfg(test)]
    pub(crate) fn fixture(path: &Path, clock: Arc<dyn TimeSource>) -> Self {
        let authority = AuthorityId::from_bytes(*b"linux-local-v001").unwrap();
        Self(SnapshotAuthority::fixture(path, authority, clock))
    }
    pub(crate) fn operation_watch_for(
        &self,
        caller: &CallerContext,
        grant: &ValidatedGrant,
    ) -> Result<carosign_linux::policy::PolicyChangeWatch, AuthorityRefusal> {
        self.0.operation_watch_for(caller, grant)
    }
    pub(crate) fn revalidate_now(
        &self,
        caller: &CallerContext,
        grant: &ValidatedGrant,
    ) -> Result<(), AuthorityRefusal> {
        self.0.revalidate_for(caller, grant)
    }
    /// Downloads rules from a protected system file.
    pub fn open(path: &Path, clock: Arc<dyn TimeSource>) -> Result<Self, AuthorityRefusal> {
        // The constant area of system UIDs, the same when restarting.
        let authority = AuthorityId::from_bytes(*b"linux-local-v001")
            .map_err(|_| AuthorityRefusal::Unavailable)?;
        SnapshotAuthority::open(path, authority, clock).map(Self)
    }

    /// The subject is allowed only by the system personality and the rules of the picture.
    pub fn identity(&self) -> &dyn IdentityAuthority {
        &self.0
    }

    /// Decides authority and oversees the change of the image.
    pub fn authority(&self) -> &dyn PolicyAuthority {
        &self.0
    }
}

impl IdentityAuthority for LinuxPolicy {
    fn resolve(
        &self,
        caller: &CallerContext,
        selector: &SubjectSelector,
    ) -> BoxFuture<'_, Result<SubjectRef, IdentityError>> {
        self.0.resolve(caller, selector)
    }

    fn resolve_verification<'a>(
        &'a self,
        caller: &'a CallerContext,
        intent: &'a VerificationIntent,
    ) -> BoxFuture<'a, Result<SubjectRef, IdentityError>> {
        self.0.resolve_verification(caller, intent)
    }
}

impl PolicyAuthority for LinuxPolicy {
    fn subject_uid(&self, subject: SubjectRef) -> Result<u32, AuthorityRefusal> {
        if !self.is_current() {
            return Err(AuthorityRefusal::Unavailable);
        }
        self.uid_of_subject(subject)
            .map_err(|_| AuthorityRefusal::Denied)
    }
    fn authorize_verification<'a>(
        &'a self,
        caller: &'a CallerContext,
        intent: &'a VerificationIntent,
        subject: SubjectRef,
    ) -> BoxFuture<'a, Result<ValidatedGrant, AuthorityRefusal>> {
        self.0.authorize_verification(caller, intent, subject)
    }

    fn authorize(
        &self,
        caller: &CallerContext,
        scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        self.0.authorize(caller, scope)
    }

    fn revalidate(
        &self,
        grant: &ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        self.0.revalidate(grant, boundary)
    }

    fn revalidate_caller<'a>(
        &'a self,
        caller: &'a CallerContext,
        grant: &'a ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'a, Result<(), AuthorityRefusal>> {
        self.0.revalidate_caller(caller, grant, boundary)
    }
}

#[path = "linux_policy.tests.rs"]
#[cfg(test)]
mod tests;
