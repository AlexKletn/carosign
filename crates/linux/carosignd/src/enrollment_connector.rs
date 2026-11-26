//! Closed endpoints of one operation issued by the system supervisor.

use carosign_core::{authorization::ValidatedGrant, device::AuthenticatorRef};
use carosign_linux_ipc::{
    enrollment_authority::{AuthorityChannel, ExpectedPeer, GrantBinding},
    enrollment_authority_adapter::TrustedEnrollmentConnector,
};
use carosign_runtime::enrollment_authorization::{
    EnrollmentAuthorizationError as Error, EnrollmentAuthorizationScope, EnrollmentRecipientId,
};
use std::{os::fd::OwnedFd, sync::Mutex, time::Duration};

pub(crate) struct OwnerConnector {
    authenticator: AuthenticatorRef,
    recipient: EnrollmentRecipientId,
    endpoints: Mutex<Option<(OwnedFd, OwnedFd)>>,
}

impl OwnerConnector {
    pub(crate) fn new(
        authenticator: AuthenticatorRef,
        recipient: EnrollmentRecipientId,
        operation: OwnedFd,
        control: OwnedFd,
    ) -> Self {
        Self {
            authenticator,
            recipient,
            endpoints: Mutex::new(Some((operation, control))),
        }
    }
}

impl TrustedEnrollmentConnector for OwnerConnector {
    fn recipient_for(
        &self,
        authenticator: AuthenticatorRef,
    ) -> Result<EnrollmentRecipientId, Error> {
        if authenticator != self.authenticator {
            return Err(Error::Invalid);
        }
        Ok(self.recipient)
    }

    fn connect(
        &self,
        scope: &EnrollmentAuthorizationScope,
        grant: &ValidatedGrant,
    ) -> Result<(AuthorityChannel, AuthorityChannel), Error> {
        if scope.authenticator != self.authenticator || scope.recipient != self.recipient {
            return Err(Error::Invalid);
        }
        let (operation, control) = self
            .endpoints
            .lock()
            .map_err(|_| Error::Unavailable)?
            .take()
            .ok_or(Error::Unavailable)?;
        // SAFETY: geteuid does not require preconditions. SO_PEERCRED It captures the creator
        // of the socketpair.
        let peer = ExpectedPeer {
            pid: std::process::id(),
            uid: unsafe { libc::geteuid() },
        };
        let binding = GrantBinding::from_grant(grant);
        let operation = AuthorityChannel::bootstrap_client(
            operation,
            peer,
            scope,
            binding.clone(),
            Vec::new(),
            Duration::from_secs(5),
        )
        .map_err(|_| Error::Unavailable)?;
        let control = AuthorityChannel::bootstrap_client(
            control,
            peer,
            scope,
            binding,
            Vec::new(),
            Duration::from_secs(5),
        )
        .map_err(|_| Error::Unavailable)?;
        Ok((operation, control))
    }
}

#[path = "enrollment_connector.tests.rs"]
#[cfg(test)]
mod tests;
