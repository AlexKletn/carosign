//! The internal owner of the protected image policy and issued permits.
//!
//! This module doesn't identify the caller, and it's not yet connected to the regular service
//! assembly. `CallerContext` I have to give you transport.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use carosign_adapters::policy::{self, FileIdentity, FilePolicy, Policy};
use carosign_core::authorization::{AuthorizationBoundary, GrantScope, ValidatedGrant};
use carosign_core::identity::{IdentityError, SubjectRef, SubjectSelector};
use carosign_core::ids::{AuthorityId, GrantId};
use carosign_linux::policy::{PolicyChangeWatch, PolicyWatchState, ProtectedPolicySnapshot};
use carosign_runtime::authority::verification::{RequestedSubject, VerificationIntent};
use carosign_runtime::ports::{
    AuthorityRefusal, BoxFuture, CallerContext, IdentityAuthority, PolicyAuthority, TimeSource,
};

mod limits;
use limits::MAX_GRANTS;

#[path = "policy_authority.tests.rs"]
#[cfg(test)]
mod tests;

mod issued;
use issued::Issued;
mod state;
use state::State;
mod snapshot_authority;
pub(crate) use snapshot_authority::SnapshotAuthority;

mod grant_match;
use grant_match::same_grant;
mod account_lookup;
use account_lookup::account_uid;
