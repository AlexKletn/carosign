use super::AdapterError;
use super::FIELD_MAX;
use carosign_protocol::carosign::client::v1::BeginVerification;
use carosign_protocol::carosign::common::v1 as common;
use core::fmt;

/// Collects a verification request; the service is obliged to allow a name prompt and
/// purpose.
pub fn verification_request(
    request: &Request,
    authenticator_id: Vec<u8>,
    idempotency_key: [u8; 16],
    purpose: common::VerificationPurpose,
) -> Result<BeginVerification, AdapterError> {
    request.validate()?;
    if authenticator_id.len() != 16
        || authenticator_id.iter().all(|byte| *byte == 0)
        || idempotency_key.iter().all(|byte| *byte == 0)
        || purpose == common::VerificationPurpose::PurposeUnspecified
    {
        return Err(AdapterError::InvalidOperation);
    }
    Ok(BeginVerification {
        report_matched_enrollment: false,
        enrollment: None,
        idempotency_key: idempotency_key.to_vec(),
        subject: Some(common::SubjectSelector {
            selector: Some(common::subject_selector::Selector::AccountHint(
                request.user.clone(),
            )),
        }),
        authenticator_id,
        purpose_hint: purpose as i32,
        authority_request_ref: Vec::new(),
    })
}

/// Adapter request.
#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    /// Who is asked to check (name from PAM, not confirmation).
    pub user: String,
    /// What PAM service is asking for.
    pub service: String,
    /// What is the name of the connection (not the authority).
    pub connection: String,
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Request { поля скрыты }")
    }
}

impl Request {
    /// Checks the fields before going to the service.
    ///
    /// ⚠️ The name from PAM comes from the reading field and can contain anything; it's a
    /// form checker, wrong.
    pub fn validate(&self) -> Result<(), AdapterError> {
        if self.user.is_empty() {
            return Err(AdapterError::EmptyUser);
        }
        for field in [&self.user, &self.service, &self.connection] {
            if field.len() > FIELD_MAX {
                return Err(AdapterError::TooLong);
            }
            if field.chars().any(char::is_control) {
                return Err(AdapterError::ControlCharacters);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "request.tests.rs"]
mod tests;
