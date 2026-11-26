use super::{AcceptError, common};

/// Refusal in the form of a wire code.
///
/// ⚠️ Three troubles, three codes: `SUBJECT_UNKNOWN` (no registrations), `UNAUTHORIZED`
/// (politics) and `UNAVAILABLE` (couldn't ask).
pub fn error_of(error: &AcceptError) -> common::Error {
    let (code, message) = match error {
        AcceptError::NotEnrolled => (common::ErrorCode::ErrorSubjectUnknown, "not enrolled"),
        AcceptError::UnknownSubject => (common::ErrorCode::ErrorSubjectUnknown, "unknown subject"),
        AcceptError::Unauthorized(_) => (common::ErrorCode::ErrorUnauthorized, "unauthorized"),
        AcceptError::IdentityUnavailable => {
            (common::ErrorCode::ErrorUnavailable, "identity unavailable")
        }
        AcceptError::Scope(_) => (common::ErrorCode::ErrorUnavailable, "scope unavailable"),
    };
    common::Error {
        code: code as i32,
        safe_message: message.to_string(),
    }
}
