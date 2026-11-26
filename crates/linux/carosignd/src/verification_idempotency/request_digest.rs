use super::{
    GrantScope, InvalidScope, PurposeHint, RequestedSubject, VerificationIntent,
    VerificationPurpose, put_field,
};

/// Hash shouldn't be in the magazine.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct RequestDigest(pub(super) [u8; 32]);

impl RequestDigest {
    /// Encodes only the parameters listed in the accepted ADR.
    pub(crate) fn new(
        intent: &VerificationIntent,
        scope: &GrantScope,
    ) -> Result<Self, InvalidScope> {
        let GrantScope::Verify {
            subject,
            authenticator,
            purpose,
        } = scope
        else {
            return Err(InvalidScope);
        };
        if *authenticator != intent.authenticator {
            return Err(InvalidScope);
        }
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"biometric.verify.idempotency.v1\0");
        bytes.extend_from_slice(&subject.to_canonical());
        bytes.extend_from_slice(authenticator.as_bytes());
        bytes.push(match purpose {
            VerificationPurpose::SessionLogin => 1,
            VerificationPurpose::SessionUnlock => 2,
            VerificationPurpose::PrivilegedAction => 3,
            VerificationPurpose::FactorSelfCheck => 4,
            VerificationPurpose::KeyOperation => 5,
        });
        let (kind, value): (u8, &[u8]) = match &intent.subject {
            RequestedSubject::Current => (1, &[]),
            RequestedSubject::AccountHint(s) => (2, s.as_bytes()),
            RequestedSubject::Opaque(s) => (3, s),
        };
        bytes.push(kind);
        put_field(&mut bytes, value)?;
        bytes.push(match intent.purpose_hint {
            PurposeHint::SessionLogin => 1,
            PurposeHint::SessionUnlock => 2,
            PurposeHint::PrivilegedAction => 3,
            PurposeHint::SelfCheck => 4,
            PurposeHint::KeyOperation => 5,
        });
        put_field(&mut bytes, &intent.authority_request_ref)?;
        if let Some(enrollment) = intent.enrollment {
            bytes.extend_from_slice(enrollment.id.as_bytes());
            bytes.extend_from_slice(&enrollment.generation.value().to_be_bytes());
        }
        if intent.report_matched_enrollment {
            bytes.push(1);
        }
        Ok(Self(carosign_persistence::mutation_plan::sha256(&bytes)))
    }
}
