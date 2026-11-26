use super::super::SubjectRef;
use super::super::SubjectSelector;
use super::IdentityError;

/// Port of the Source of Identity: The Only Legal Way to
/// Receive`SubjectRef`](super::super::SubjectRef) from an untested choice.
///
/// ⚠️ Trait is here announced, but not implemented: the incarnation lives in
/// `carosign-runtime`/`carosign-linux` He knows about Linux uid and profiles.
/// `core` He doesn't know about any of this.
pub trait IdentityAuthority {
    /// Allows the choice of the subject from the caller.
    ///
    /// `selector` Unverified data from the network, not authority.
    fn resolve_subject(&self, selector: &SubjectSelector) -> Result<SubjectRef, IdentityError>;
}
