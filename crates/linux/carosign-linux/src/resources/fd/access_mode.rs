use super::HandleKind;

/// What the kernel allows you to do with the descriptor.
///
/// ⚠️ It is issued at the opening, and ** not ** when giving: if the rights are already wide,
/// you can not give them at all.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AccessMode {
    /// You can read.
    pub read: bool,
    /// You can pee.
    pub write: bool,
    /// You can go through it (directories).
    pub directory: bool,
    /// You can not change the content (for the file in memory).
    pub sealed: bool,
}

impl AccessMode {
    /// Just reading.
    pub const READ_ONLY: Self = Self {
        read: true,
        write: false,
        directory: false,
        sealed: false,
    };

    /// Reading and writing.
    pub const READ_WRITE: Self = Self {
        read: true,
        write: true,
        directory: false,
        sealed: false,
    };

    /// Is this access suitable for such a role?
    ///
    /// ⚠️ Adjustment ()`Config`) must be immutable and non-directory: the changeable setting
    /// is the editing channel, not the data.
    pub fn fits(self, kind: HandleKind) -> bool {
        match kind {
            HandleKind::CharacterDevice => self.read || self.write,
            HandleKind::SocketPair => self.read && self.write && !self.directory,
            HandleKind::SealedMemfd => self.read && !self.write && self.sealed && !self.directory,
            HandleKind::EventFd => self.read && self.write && !self.directory,
        }
    }
}
