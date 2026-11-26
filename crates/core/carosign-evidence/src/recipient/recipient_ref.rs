use core::fmt;

/// Reference to the recipient.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecipientRef {
    id: [u8; 16],
}

impl RecipientRef {
    /// Recipient.
    pub fn new(id: [u8; 16]) -> Option<Self> {
        if id == [0u8; 16] {
            return None;
        }
        Some(Self { id })
    }

    pub fn id(&self) -> [u8; 16] {
        self.id
    }
}

impl fmt::Debug for RecipientRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecipientRef(..)")
    }
}
