//! Customer request > domain request.
//!
//! ⚠️ No field is considered a right: `authority_request_ref` - prompt, not permission; the
//! key to idempotence is customer data.

mod domain_request;
mod mapping_error;

pub use domain_request::DomainRequest;
pub use domain_request::to_domain;
pub use mapping_error::MappingError;
