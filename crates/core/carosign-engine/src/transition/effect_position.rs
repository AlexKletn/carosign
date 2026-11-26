use core::fmt;

/// The position of the effect within the transition.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectPosition(u32);

impl EffectPosition {
    /// Position 0 is the first transition effect.
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for EffectPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EffectPosition({})", self.0)
    }
}
