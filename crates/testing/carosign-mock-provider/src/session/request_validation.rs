use carosign_protocol::carosign::{common::v1 as common, provider::v1 as provider};

pub(super) fn valid_id(value: &[u8]) -> bool {
    value.len() == 16 && value.iter().any(|byte| *byte != 0)
}

pub(crate) fn valid_verification(
    request: &provider::StartVerification,
    authenticator: &common::AuthenticatorRef,
) -> bool {
    let Some(subject) = &request.subject else {
        return false;
    };
    if request.authenticator.as_ref() != Some(authenticator)
        || !valid_id(&subject.authority_id)
        || !valid_id(&subject.subject_id)
        || !valid_id(&request.challenge_ref)
        || !valid_id(&request.recipient_ref)
        || !(1..=5).contains(&request.purpose)
        || request.permitted_enrollments.is_empty()
        || request.permitted_enrollments.len()
            > carosign_protocol::limits::ENROLLMENTS_PER_RESPONSE_MAX
    {
        return false;
    }
    let mut ids = std::collections::BTreeSet::new();
    for binding in &request.permitted_enrollments {
        let Some(reference) = &binding.enrollment else {
            return false;
        };
        if !valid_id(&reference.enrollment_id)
            || reference.authenticator_id != authenticator.authenticator_id
            || reference.generation == 0
            || !ids.insert(&reference.enrollment_id)
            || binding.kind
                != provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32
            || binding.provider_opaque_ref.len() > carosign_protocol::limits::OPAQUE_REF_MAX
            || binding.format_id.len() > carosign_protocol::limits::FORMAT_ID_MAX
        {
            return false;
        }
    }
    true
}
