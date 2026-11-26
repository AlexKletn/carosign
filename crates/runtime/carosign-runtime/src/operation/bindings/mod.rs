//! A set of authorized registrations is something without which verification is prohibited.
//!
//! ⚠️ Rule 29 (D-03): Verification is addressed to a specific * * set of registrations. Empty
//! set is not “yet empty”, but an excuse to refuse: `no unscoped 'scan all chip slots'
//! fallback`.
//!
//! ⚠️ The bundle (`binding`) This is not an authority. ** The right is to issue the platform;
//! the bundle only tells the provider what ** registrations it is entitled to consider.

mod binding_error;
mod binding_kind;
mod enrollment_binding;
mod limits;
mod permitted_set;
mod provider_reference;
mod set_error;

pub use binding_error::BindingError;
pub use binding_kind::BindingKind;
pub use enrollment_binding::EnrollmentBinding;
pub use limits::BINDINGS_MAX;
pub use permitted_set::PermittedSet;
pub use provider_reference::ProviderReference;
pub use set_error::SetError;
