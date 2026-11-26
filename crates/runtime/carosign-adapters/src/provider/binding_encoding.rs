use carosign_protocol::carosign::{common::v1 as common, provider::v1 as provider};

/// Domain link → wire link.
pub(super) fn enrollment_ref(
    reference: carosign_core::enrollment::EnrollmentRef,
) -> common::EnrollmentRef {
    common::EnrollmentRef {
        enrollment_id: reference.id.as_bytes().to_vec(),
        authenticator_id: reference.authenticator.as_bytes().to_vec(),
        generation: reference.generation.value(),
    }
}

/// Domain bundle → wire bundle.
pub(super) fn to_binding(
    binding: &carosign_runtime::EnrollmentBinding,
) -> provider::EnrollmentBinding {
    provider::EnrollmentBinding {
        enrollment: Some(enrollment_ref(binding.enrollment)),
        kind: match binding.kind {
            carosign_runtime::BindingKind::ProviderOpaque => {
                provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32
            }
            carosign_runtime::BindingKind::ScopedVault => {
                provider::EnrollmentBindingKind::EnrollmentBindingScopedVault as i32
            }
        },
        provider_opaque_ref: binding.provider_ref.clone(),
        format_id: binding.format_id.clone().unwrap_or_default(),
        format_version: binding.format_version,
    }
}
