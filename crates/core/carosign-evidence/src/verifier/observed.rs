use super::ProtectedAnswer;
use core::fmt;

/// What the world watched around the evidence.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Observed {
    /// To whom the proof is addressed.
    pub recipient: [u8; 16],
    /// What a challenge they answered.
    pub challenge: [u8; 16],
    /// Was there a hardware path?
    pub hardware_path: bool,
    /// What the secure module said.
    pub protected: ProtectedAnswer,
}

impl fmt::Debug for Observed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Observed")
            .field("hardware_path", &self.hardware_path)
            .field("protected", &self.protected)
            .finish_non_exhaustive()
    }
}
