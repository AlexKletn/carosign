use crate::generated::carosign::provider::v1 as provider;
use crate::limits;

use super::super::{WireError, bounded, bounded_str, require_id};

/// Verification of the outcome of the inspection.
pub fn check_verification_outcome(
    outcome: &provider::VerificationOutcome,
) -> Result<(), WireError> {
    use provider::VerificationStatus;
    use provider::verification_outcome::Evidence;

    if outcome.status == VerificationStatus::VerifyStatusUnspecified as i32 {
        return Err(WireError::UnspecifiedEnum {
            field: "verification_outcome.status",
        });
    }
    if VerificationStatus::from_i32(outcome.status).is_none() {
        return Err(WireError::UnknownEnum {
            field: "verification_outcome.status",
            value: outcome.status,
        });
    }
    bounded_str(
        &outcome.evidence_format_id,
        "evidence_format_id",
        limits::FORMAT_ID_MAX,
    )?;
    if let Some(enrollment) = &outcome.matched_enrollment {
        if outcome.status != VerificationStatus::VerifyMatched as i32 {
            return Err(WireError::UnexpectedPayload {
                what: "регистрация при несовпадении",
            });
        }
        require_id(
            &enrollment.enrollment_id,
            "matched_enrollment.enrollment_id",
        )?;
        require_id(
            &enrollment.authenticator_id,
            "matched_enrollment.authenticator_id",
        )?;
        if enrollment.generation == 0 {
            return Err(WireError::BadId {
                field: "matched_enrollment.generation",
            });
        }
    }
    match outcome.status {
        status if status == VerificationStatus::VerifyMatched as i32 => match &outcome.evidence {
            Some(Evidence::EvidenceRef(reference)) => {
                require_id(reference, "evidence_ref")?;
            }
            Some(Evidence::OpaqueToken(token)) => {
                bounded(token, "opaque_token", limits::OPAQUE_TOKEN_MAX)?;
            }
            // Coincidence without a token is the provider’s approval; the trusted proof
            // separately creates runtime after checking the context.
            None => {}
        },
        _ => {
            if outcome.evidence.is_some() {
                return Err(WireError::UnexpectedPayload {
                    what: "доказательство при неуспешном исходе",
                });
            }
        }
    }
    Ok(())
}
