//! Resource mediator: policy, extradition, descriptors and denial.
//!
//! ⚠️ The rule of document 09, for which this kreit exists, is that the descriptor is wrong
//! in itself. The broker only gives out what is declared a policy and issued in live
//! leasing, and the descriptor given can only be withdrawn by completing the process.

pub mod external_body;
pub mod fd_policy;
pub mod owner_channel;
pub mod policy;
pub mod process;
pub mod recovery;
mod relay;
pub mod relay_dispatch;
// The queue is not yet connected to the performer: the process remains fail-closed.
#[allow(dead_code)]
mod relay_pump;
#[allow(dead_code)]
mod relay_queue;
pub mod resources;
pub mod server;
pub mod transport;
