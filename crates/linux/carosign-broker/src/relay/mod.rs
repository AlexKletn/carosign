//! Check the initial tunnel boundary; the source of trust defines the composition.
//!
//! So far, only Hello is accepted here. It's not a commandeer or permission to transfer a
//! resource or run a production broker.

mod hello_gate;
mod refusal;
mod shutdown_gate;
#[cfg(test)]
mod test_fixture;
mod tunnel_handshake;

pub(crate) use hello_gate::HelloGate;
pub(crate) use hello_gate::request_id;
pub(crate) use refusal::Refusal;
#[allow(unused_imports)]
pub(crate) use shutdown_gate::ShutdownGate;
