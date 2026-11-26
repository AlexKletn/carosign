use carosign_protocol::carosign::common::v1 as common;

use crate::scenario::Scenario;

pub(super) fn capabilities(scenario: Scenario) -> common::CapabilityDescriptor {
    common::CapabilityDescriptor {
        modality: common::Modality::Fingerprint as i32,
        operations: vec![
            common::ProviderOperation::ProviderVerify as i32,
            common::ProviderOperation::ProviderEnroll as i32,
            common::ProviderOperation::ProviderRemove as i32,
        ],
        template_custody: common::TemplateCustody::CustodyProviderManaged as i32,
        interaction_modes: vec![common::InteractionMode::InteractionTouch as i32],
        recovery_mode: common::RecoveryMode::RecoveryNotSupported as i32,
        template_format_id: String::new(),
        template_format_version: 0,
        cancel_request_supported: matches!(
            scenario,
            Scenario::PendingCancel | Scenario::SeededPendingCancel | Scenario::LinuxPendingCancel
        ) || cfg!(feature = "ssh-mock"),
        enrollment_authorization: common::EnrollmentAuthorizationMode::EnrollAuthNone as i32,
        supported_events: if matches!(scenario, Scenario::LinuxPersistent) {
            vec![common::CaptureEventKind::CaptureEventState as i32]
        } else {
            Vec::new()
        },
        ..Default::default()
    }
}
