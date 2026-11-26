//! Receiving requests: from the frame of the client to the operation.
//!
//! ⚠️ Rule 05/22: The service does not make decisions - it collects a set from the registry,
//! asks for authority and gives to the actor.
//!
//! ⚠️ Three rejections that should sound different:
//!
//! * no registrations (`NotEnrolled`- the person simply does not have them;
//! * authority not granted (`Unauthorized`) - the policy has prohibited;
//! * port inaccessible`Unavailable`) - couldn't ask.
//!
//! One code for three troubles is exactly what it shouldn't be.

use carosign_core::authorization::{GrantScope, ValidatedGrant};
use carosign_core::identity::{SubjectRef, SubjectSelector};
use carosign_core::ids::{AuthenticatorId, OperationId};
use carosign_persistence::store::{MetadataStore, Table};
use carosign_runtime::VerificationCycle;
use carosign_runtime::authority::verification::{
    VerificationAuthorityError, VerificationIntent, authorize_verification,
};
use carosign_runtime::authority::{Candidate, SelectionError, select};
use carosign_runtime::ports::{AuthorityRefusal, CallerContext, PolicyAuthority};

mod accept_error;
pub use accept_error::AcceptError;

#[path = "accept.tests.rs"]
#[cfg(test)]
mod tests;

mod verification_admission;
pub(crate) use verification_admission::prepare_authorized_verification;
pub use verification_admission::prepare_verification;
pub(crate) use verification_admission::{
    authorize_client_intent, prepare_client_verification, prepare_verified_intent,
};
mod enrollment_lookup;
use enrollment_lookup::read_candidates;
pub(crate) use enrollment_lookup::read_enrollment_records;
mod request_scope;
pub use request_scope::operation_id;
pub use request_scope::scope_of;
pub use request_scope::selector_is_current;
