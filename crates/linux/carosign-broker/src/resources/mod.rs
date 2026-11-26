//! Resources: application, employment graph, issuance and descriptors.
//!
//! ⚠️ The main thing about trust (Document 09): **The descriptor itself is not right.
//! You can give it only for what has already been issued in leasing; and after giving it
//! back, the “logical review” does not stop it – it stops the bridge to the process.
//!
//! ⚠️ It's just data and solutions. `SCM_RIGHTS` and `pidfd` live `carosign-linux`; this
//! kreit doesn't open them.

mod binding;
mod binding_context;
mod claim;
mod claim_error;
mod fd_role;
mod resource_kind;

pub use binding::Binding;
pub use binding::role_fits;
pub use binding::validate;
pub use binding_context::BindingContext;
pub use claim::Claim;
pub use claim_error::ClaimError;
pub use fd_role::FdRole;
pub use resource_kind::ResourceKind;
