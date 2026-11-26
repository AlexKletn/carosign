//! The actor of the operation: the only writer of his fortune.
//!
//! ⚠️ Rule 22: The state of an operation is changed only by the actor, and only by accepting
//! it.`Decision`](carosign_engine::Decision)The others send surveillance to the mail.
//!
//! ⚠️ There's none here. `await`: the actor is clean. Anything that takes time does the
//! effector and brings back the observation, so locking the actor can't wait for the socket.

mod observation_error;
mod operation_actor;
mod operation_msg;
mod step;

pub use observation_error::ObservationError;
pub use operation_actor::OperationActor;
pub use operation_msg::OperationMsg;
pub use step::Step;
