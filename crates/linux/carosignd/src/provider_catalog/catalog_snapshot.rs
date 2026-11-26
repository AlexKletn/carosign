use carosign_core::device::AuthenticatorRef;
use carosign_protocol::carosign::client::v1::Authenticators;

pub(super) struct CatalogSnapshot {
    pub(super) inventory: Authenticators,
    pub(super) candidates: Vec<AuthenticatorRef>,
}
