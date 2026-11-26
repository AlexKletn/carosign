use super::super::AccountHint;
use super::InvalidSelector;
use core::fmt;

/// What the caller asks for when choosing the subject.
///
/// ⚠️ Field. `account_hint` The port is obliged to check it with whoever the caller's order
/// actually allows.
#[derive(Clone, PartialEq, Eq)]
pub enum SubjectSelector {
    /// The subject of the current caller is assigned through the context of the call.
    Current,
    /// Source prompt ("account name"); allows port.
    AccountHint(AccountHint),
    /// Non-transparent choice at the source of trust, up to 256 bytes.
    Opaque(Vec<u8>),
}

impl fmt::Debug for SubjectSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Current => "SubjectSelector::Current",
            Self::AccountHint(_) => "SubjectSelector::AccountHint(..)",
            Self::Opaque(_) => "SubjectSelector::Opaque(..)",
        })
    }
}

/// Maximum length of opaque subject selection, in bytes.
pub const OPAQUE_SELECTOR_MAX: usize = 256;

impl SubjectSelector {
    /// Checks for opaque choice of length and zeros.
    ///
    /// ⚠️ The success of the test does not mean that the choice is confirmed by something: it
    /// is only the reception of data.
    pub fn opaque(bytes: Vec<u8>) -> Result<Self, InvalidSelector> {
        if bytes.is_empty() || bytes.len() > OPAQUE_SELECTOR_MAX {
            return Err(InvalidSelector);
        }
        Ok(Self::Opaque(bytes))
    }
}
