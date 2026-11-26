//! Connection to the platform: one frame, one message.
//!
//! ⚠️ The frame is limited to 64 KB ** to parsing. Exceeding is a failure, not a "browse and
//! try": a cropped message can mean something very different.
//!
//! ⚠️ The SDK does not open sockets and does not select addresses: the channel gives out the
//! platform.
//! There are only rules of conversation on the already issued channel.

mod channel;
mod frame_guard;
mod transport;
mod transport_error;

pub use channel::Channel;
pub use channel::payload_name;
use frame_guard::FrameGuard;
pub use transport::Transport;
pub use transport_error::TransportError;
