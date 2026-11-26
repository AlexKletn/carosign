//! Cancellation: Request and receipt are different things.
//!
//! ⚠️ The worker responds `CancelRequested`But you don't have to stop the hardware, because a
//! heavy call can go. A receipt is not a stop (document 06).
//!
//! ⚠️ Reading the cancellation channel lives **separate ** from the move reader, otherwise
//! the request for cancellation will stand in line for the shooting - and can only come to
//! the end.

mod cancel_not_quiesced;
mod cancel_state;

pub use cancel_not_quiesced::CancelNotQuiesced;
pub use cancel_state::CancelState;
