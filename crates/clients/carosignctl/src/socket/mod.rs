//! Diagnostic client; the socket's response is not authorized.

mod authenticator_list;
mod execute;
mod inventory;
mod mutation;
mod operation_id;
mod operation_status;
mod transport_error;
mod verification;

pub(crate) use execute::execute;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
