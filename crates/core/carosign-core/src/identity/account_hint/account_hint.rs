use super::InvalidAccountHint;
use core::fmt;

/// Account Hint: A non-empty UTF-8 string is no longer than 64 bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct AccountHint(String);

impl fmt::Debug for AccountHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The name of the account is personal data; it does not go to the log.
        f.write_str("AccountHint(..)")
    }
}

impl AccountHint {
    /// Checks and accepts the clue.
    ///
    /// The clue remains **data**: it does not confirm that such an account exists or belongs
    /// to the caller.
    pub fn new(value: &str) -> Result<Self, InvalidAccountHint> {
        if value.is_empty() || value.len() > 64 {
            return Err(InvalidAccountHint);
        }
        if value.chars().any(char::is_control) {
            return Err(InvalidAccountHint);
        }
        Ok(Self(value.to_owned()))
    }

    /// Like it was passed on.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
