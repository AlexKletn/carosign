use crate::carosign::common::v1::UnavailabilityReason;

impl UnavailabilityReason {
    pub fn message_key(self) -> Option<&'static str> {
        Some(match self {
            Self::Unspecified => return None,
            Self::InventoryOnly => "device-reason-inventory-only",
            Self::ProviderUnreachable => "device-reason-provider-unreachable",
            Self::DeviceNotPublished => "device-reason-not-published",
            Self::ProviderReportedUnavailable => "device-reason-provider-reported",
            Self::ExecutionProfileRejected => "device-reason-profile-rejected",
            Self::ModalityMismatch => "device-reason-modality-mismatch",
            Self::UnsupportedCapabilities => "device-reason-unsupported",
            Self::ReadinessCheckUnconfigured => "device-reason-check-unconfigured",
            Self::ReadinessCheckPending => "device-reason-check-pending",
            Self::ReadinessCheckFailed => "device-reason-check-failed",
            Self::ResourceFenced => "device-reason-resource-fenced",
            Self::DisabledByAdministrator => "device-reason-disabled-by-admin",
        })
    }
}
