use core::fmt;

/// A call link.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChallengeRef {
    id: [u8; 16],
    single_use: bool,
}

impl ChallengeRef {
    /// A call received by a protected recipient.
    ///
    /// ⚠️ `single_use` A mandatory property, not a wish flag: a challenge that can be
    /// presented twice does not protect against overplay.
    pub fn accepted_by_recipient(id: [u8; 16]) -> Option<Self> {
        if id == [0u8; 16] {
            return None;
        }
        Some(Self {
            id,
            single_use: true,
        })
    }

    pub fn id(&self) -> [u8; 16] {
        self.id
    }

    /// One-time.
    pub fn is_single_use(&self) -> bool {
        self.single_use
    }
}

impl fmt::Debug for ChallengeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChallengeRef(..)")
    }
}
