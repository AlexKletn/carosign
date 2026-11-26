//! The subject and owner of the operation.
//!
//! ⚠️ `SubjectRef` - **a couple* `(AuthorityId, SubjectId)`Equality and hash must take into
//! account both fields (decision R-04 of package v0.6.1): two sources of trust should not
//! collide on the same `SubjectId`Otherwise, the records of one profile will go to another.
//!
//! There is no Unix call here: Linux uid, Veles profile, and names to show live in the
//! platform display.`carosign-linux`) and the provider receives the subject only by an
//! opaque reference.

mod account_hint;
mod authority;
mod owner_ref;
mod selector;
mod subject_ref;

pub use account_hint::{AccountHint, InvalidAccountHint};
pub use authority::{IdentityAuthority, IdentityError};
pub use owner_ref::OwnerRef;
pub use selector::{InvalidSelector, OPAQUE_SELECTOR_MAX, SubjectSelector};
pub use subject_ref::{InvalidSubject, SUBJECT_REF_LEN, SUBJECT_SCOPE_PREFIX_LEN, SubjectRef};

#[path = "identity.tests.rs"]
#[cfg(test)]
mod tests;
