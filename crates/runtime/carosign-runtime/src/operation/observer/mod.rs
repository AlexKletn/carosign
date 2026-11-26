//! Observers: watching, not possessing.
//!
//! ⚠️ Disabling the observer does not cancel anything (A-03).The observer is a subscription,
//! not a participant: his departure should not change the fate of the operation.

mod authorized_subscription;
mod limits;
mod observer;
mod observer_error;
mod observer_queue;
mod observer_queue_error;
mod observers;
mod subscription_error;
mod terminal_outcome_port;

pub use authorized_subscription::AuthorizedSubscription;
pub use authorized_subscription::authorize;
pub use limits::OBSERVER_QUEUE_MAX;
pub use limits::{OBSERVERS_HARD_MAX, OBSERVERS_MAX};
pub use observer::Observer;
pub use observer_error::ObserverError;
pub use observer_queue::ObserverQueue;
pub use observer_queue_error::ObserverQueueError;
pub use observers::Observers;
pub use subscription_error::SubscriptionError;
pub use terminal_outcome_port::TerminalOutcomePort;
