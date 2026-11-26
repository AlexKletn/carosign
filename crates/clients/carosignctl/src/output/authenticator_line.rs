use core::fmt;

/// The string of the authenticator list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatorLine {
    /// A short human ID.
    pub short_id: String,
    pub available: bool,
}

impl fmt::Display for AuthenticatorLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}  ", self.short_id)?;
        f.write_str(if self.available {
            "доступен"
        } else {
            "недоступен"
        })
    }
}
