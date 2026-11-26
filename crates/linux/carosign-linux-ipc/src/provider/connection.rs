//! Connection with the worker: the channel issued and its dimensions.

/// Channel characteristics.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Channel {
    /// Maximum shot.
    pub max_frame: usize,
    /// Maximum descriptors.
    pub max_fds: usize,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            max_frame: carosign_protocol::limits::FRAME_MAX,
            max_fds: carosign_protocol::limits::FD_SLOTS_MAX,
        }
    }
}
