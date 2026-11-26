//! Servicing the call of the broker.
//!
//! ⚠️ There are three rules (document 26) without which the broker becomes a hole: 1.
//! the call is checked against **live** leasing and **current** era; 2. the handler is taken
//! from the exact three (packet, resource, handler), and not from the text of the worker;
//! 3. the changing call requires confirmed permission on the disk.
//!
//! ⚠️ There is not one “let go as is”: the unknown is a refusal.

mod broker;
mod call;
mod call_result;
mod dispatch_authorization;

pub use broker::Broker;
pub use call::Call;
pub use call_result::CallResult;
pub use dispatch_authorization::DispatchAuthorization;
