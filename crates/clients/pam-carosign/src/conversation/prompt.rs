use core::fmt;

/// What to show.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prompt {
    /// "Put your finger on it."
    PlaceFinger,
    /// "Don't let go."
    HoldFinger,
    /// "It didn't match, again."
    Retry,
    /// "Working."
    Working,
}

impl Prompt {
    pub fn localized(self, localizer: &carosign_l10n::Localizer) -> String {
        localizer.text(self.message_key())
    }

    pub fn text(self) -> String {
        self.localized(&carosign_l10n::Localizer::from_environment())
    }

    fn message_key(self) -> &'static str {
        match self {
            Self::PlaceFinger => "place-finger",
            Self::HoldFinger => "hold-finger",
            Self::Retry => "retry-capture",
            Self::Working => "working",
        }
    }
}

impl fmt::Display for Prompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}
