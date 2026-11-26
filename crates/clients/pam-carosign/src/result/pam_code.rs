use super::Outcome;
use core::fmt;

/// The PAM codes we need.
///
/// ⚠️ Values are part of a binary contract with `libpam`Here they coincide with the system.
/// `pam.h`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PamCode {
    Success,
    /// Wrong user.
    UserUnknown,
    /// Denial of verification.
    AuthErr,
    /// Verification is not available (an alternate path is allowed).
    AuthInfoUnavail,
    /// Service or system error.
    SystemErr,
}

impl PamCode {
    /// Importance to `libpam`.
    pub fn raw(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::UserUnknown => 10,
            Self::AuthErr => 7,
            Self::AuthInfoUnavail => 9,
            Self::SystemErr => 4,
        }
    }
}

impl fmt::Debug for PamCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Success => "PAM_SUCCESS",
            Self::UserUnknown => "PAM_USER_UNKNOWN",
            Self::AuthErr => "PAM_AUTH_ERR",
            Self::AuthInfoUnavail => "PAM_AUTHINFO_UNAVAIL",
            Self::SystemErr => "PAM_SYSTEM_ERR",
        })
    }
}

/// Translates the output into PAM code.
///
/// This is only a codesheet, not a check of trust in the service or matching the result to a
/// PAM call. You can use it for success only after these checks are performed by a trusted
/// team.
pub fn to_code(outcome: Outcome) -> PamCode {
    match outcome {
        Outcome::FactorAccepted => PamCode::Success,
        Outcome::NotMatched => PamCode::AuthErr,
        Outcome::Cancelled | Outcome::Unavailable | Outcome::DaemonError => {
            PamCode::AuthInfoUnavail
        }
        Outcome::UnknownSubject => PamCode::UserUnknown,
        Outcome::InvalidEvidence | Outcome::Indeterminate => PamCode::AuthInfoUnavail,
    }
}

#[path = "pam_code.tests.rs"]
#[cfg(test)]
mod tests;
