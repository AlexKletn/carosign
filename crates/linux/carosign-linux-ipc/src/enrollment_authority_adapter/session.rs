use super::phase::Phase;
use crate::enrollment_authority::AuthorityChannel;
use crate::enrollment_authority::GrantBinding;
use carosign_core::authorization::GrantScope;
use carosign_runtime::enrollment_authorization::EnrollmentAuthorizationScope;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

pub(super) struct Session {
    pub(super) scope: EnrollmentAuthorizationScope,
    pub(super) grant: GrantBinding,
    pub(super) grant_scope: GrantScope,
    pub(super) operation: Mutex<AuthorityChannel>,
    pub(super) control: Mutex<AuthorityChannel>,
    pub(super) phase: Mutex<Phase>,
    pub(super) owner_binding: Mutex<Option<Vec<u8>>>,
    pub(super) revoked: AtomicBool,
    pub(super) revoke_confirmed: AtomicBool,
}
