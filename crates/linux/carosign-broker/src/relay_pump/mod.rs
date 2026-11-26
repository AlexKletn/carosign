//! Execution of queues with saving packets at reverse pressure.
//!
//! The caller is required to check wire and credentials and serialize the review recheck.

mod checked_request;
mod packet_relay;
mod receive_request;
mod sent_receipt;

pub(crate) use checked_request::CheckedRequest;
pub(crate) use packet_relay::RelayPump;
pub(crate) use receive_request::ReceiveRequest;
pub(crate) use sent_receipt::SentReceipt;

#[cfg(test)]
#[path = "packet_relay.tests.rs"]
mod tests;
