use super::InvalidIntent;
use super::PurposeHint;
use super::RequestedSubject;
use super::VerificationAuthorityError;
use crate::ports::CallerContext;
use crate::ports::IdentityAuthority;
use crate::ports::PolicyAuthority;
use carosign_core::authorization::AuthorizationBoundary;
use carosign_core::authorization::GrantScope;
use carosign_core::authorization::ValidatedGrant;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::AuthenticatorId;
use carosign_protocol::carosign::common::v1 as common;
use carosign_protocol::limits;
use carosign_protocol::validation;

/// Fields that both trusted sources see; neither is a right.
#[derive(Clone, PartialEq, Eq)]
pub struct VerificationIntent {
    /// Customer choice.
    pub subject: RequestedSubject,
    /// Target tip.
    pub purpose_hint: PurposeHint,
    /// Reference to the source of authority.
    pub authority_request_ref: Vec<u8>,
    /// Requested authenticator.
    pub authenticator: AuthenticatorId,
    pub enrollment: Option<carosign_core::enrollment::EnrollmentRef>,
    pub report_matched_enrollment: bool,
}

impl core::fmt::Debug for VerificationIntent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("VerificationIntent(..)")
    }
}

impl VerificationIntent {
    /// Checks wire fields, keeping them without giving authority.
    pub fn from_wire(
        subject: &common::SubjectSelector,
        purpose_hint: i32,
        authority_request_ref: &[u8],
        authenticator: AuthenticatorId,
    ) -> Result<Self, InvalidIntent> {
        validation::check_subject_selector(subject).map_err(|_| InvalidIntent)?;
        if authority_request_ref.len() > limits::AUTHORITY_REQUEST_REF_MAX {
            return Err(InvalidIntent);
        }
        use common::subject_selector::Selector;
        let subject = match &subject.selector {
            Some(Selector::CurrentSubject(true)) => RequestedSubject::Current,
            Some(Selector::AccountHint(value)) => RequestedSubject::AccountHint(value.clone()),
            Some(Selector::OpaqueAuthoritySelector(value)) => {
                RequestedSubject::Opaque(value.clone())
            }
            _ => return Err(InvalidIntent),
        };
        use common::VerificationPurpose as WirePurpose;
        let purpose_hint = match WirePurpose::from_i32(purpose_hint).ok_or(InvalidIntent)? {
            WirePurpose::PurposeSessionLogin => PurposeHint::SessionLogin,
            WirePurpose::PurposeSessionUnlock => PurposeHint::SessionUnlock,
            WirePurpose::PurposePrivilegedAction => PurposeHint::PrivilegedAction,
            WirePurpose::PurposeSelfCheck => PurposeHint::SelfCheck,
            WirePurpose::PurposeKeyOperation => PurposeHint::KeyOperation,
            WirePurpose::PurposeUnspecified => return Err(InvalidIntent),
        };
        Ok(Self {
            subject,
            purpose_hint,
            authority_request_ref: authority_request_ref.to_vec(),
            authenticator,
            enrollment: None,
            report_matched_enrollment: false,
        })
    }
}

/// Allows the subject and authority and double-checks it prior to the creation of the
/// operation.
pub async fn authorize_verification(
    identity: &dyn IdentityAuthority,
    policy: &dyn PolicyAuthority,
    caller: &CallerContext,
    intent: &VerificationIntent,
) -> Result<(SubjectRef, ValidatedGrant), VerificationAuthorityError> {
    let subject = identity
        .resolve_verification(caller, intent)
        .await
        .map_err(VerificationAuthorityError::Identity)?;
    let grant = policy
        .authorize_verification(caller, intent, subject)
        .await
        .map_err(VerificationAuthorityError::Policy)?;
    match grant.scope() {
        GrantScope::Verify {
            subject: granted_subject,
            authenticator,
            ..
        } if *granted_subject == subject && *authenticator == intent.authenticator => {}
        _ => return Err(VerificationAuthorityError::ScopeMismatch),
    }
    policy
        .revalidate_caller(caller, &grant, AuthorizationBoundary::BeforeCreation)
        .await
        .map_err(VerificationAuthorityError::Policy)?;
    Ok((subject, grant))
}
