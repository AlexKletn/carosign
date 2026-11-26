pub use crate::frame::{FrameError, check_descriptors};

pub fn may_carry_descriptors(message: &str) -> bool {
    message == "BindAuthenticator"
}

#[path = "fd_slots.tests.rs"]
#[cfg(test)]
mod tests;
