use super::InteractionMode;
use super::RecoveryMode;
use super::TemplateCustody;
use crate::modality::Modality;
use core::fmt;

/// Declared abilities are data, not evidence.
#[derive(Clone, PartialEq, Eq)]
pub struct CapabilityDescriptor {
    pub modality: Modality,
    /// He's able to check on the named subject.
    pub verify: bool,
    /// He can register.
    pub enroll: bool,
    /// He can delete.
    pub remove: bool,
    pub custody: TemplateCustody,
    /// How to restore the state.
    pub recovery: RecoveryMode,
    /// As the user touches.
    pub interaction: Vec<InteractionMode>,
    /// Accepts cancellation.
    pub cancel_request_supported: bool,
    /// Raw footage does not leave the provider.
    pub raw_capture_private: bool,
}

impl fmt::Debug for CapabilityDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapabilityDescriptor")
            .field("modality", &self.modality)
            .field("custody", &self.custody)
            .field("recovery", &self.recovery)
            .finish_non_exhaustive()
    }
}

impl CapabilityDescriptor {
    /// The smallest v1 suite: check, register and delete a single-touch fingerprint.
    ///
    /// ⚠️ It's an ad template, not an issue of authority.
    pub fn fingerprint_touch() -> Self {
        Self {
            modality: Modality::Fingerprint,
            verify: true,
            enroll: true,
            remove: true,
            custody: TemplateCustody::PlatformManaged,
            recovery: RecoveryMode::NotSupported,
            interaction: vec![InteractionMode::Touch],
            cancel_request_supported: true,
            raw_capture_private: true,
        }
    }

    /// If I could ask for a check-up.
    pub fn can_verify(&self) -> bool {
        self.verify && self.modality == Modality::Fingerprint
    }

    /// Can I ask for registration?
    pub fn can_enroll(&self) -> bool {
        self.enroll && self.modality == Modality::Fingerprint
    }

    /// Can I ask for deletion?
    pub fn can_remove(&self) -> bool {
        self.remove && self.modality == Modality::Fingerprint
    }
}
