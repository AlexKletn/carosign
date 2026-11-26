//! Limited queues of accepted tunnel; authority checks remain outside.
//!
//! This is a building block, not a working relay: queuing does not confirm sending and does
//! not give permission to work with the resource.

mod queue_error;
mod queued_packet;
mod relay_queues;

pub(crate) use queue_error::QueueError;
pub(crate) use queued_packet::QueuedPacket;
pub(crate) use relay_queues::RelayQueues;
