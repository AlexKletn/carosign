use super::ContextError;

use carosign_protocol::carosign::common::v1 as common;
use carosign_protocol::carosign::provider::v1 as provider;

/// What the worker knows about his tie-up.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkerContext {
    /// Process instance identifier (issued by the platform).
    pub instance: Vec<u8>,
    /// Package (issued by platform).
    pub package: Vec<u8>,
    /// Device profile (issued by platform).
    pub profile: Vec<u8>,
    /// The agreed minor version of the protocol.
    pub minor: u32,
    /// Included features (those that both parties know).
    pub features: Vec<provider::Feature>,
    /// A tethered machine, if the tether was already there.
    pub authenticator: Option<common::AuthenticatorRef>,
}

impl WorkerContext {
    /// Gathers context from `Initialize`.
    pub fn from_initialize(initialize: &provider::Initialize) -> Result<Self, ContextError> {
        let frame = provider::ProviderFrame {
            major: carosign_protocol::limits::MAJOR,
            minor: initialize.selected_minor,
            frame_seq: 1,
            worker_instance_id: initialize.worker_instance_id.clone(),
            payload: Some(provider::provider_frame::Payload::Initialize(
                initialize.clone(),
            )),
            ..Default::default()
        };
        if !(carosign_protocol::limits::MINOR_BASELINE..=carosign_protocol::limits::MINOR_MAX)
            .contains(&initialize.selected_minor)
            || carosign_protocol::validation::check_provider_envelope(&frame).is_err()
        {
            return Err(ContextError::MissingInstance);
        }
        Ok(Self {
            instance: initialize.worker_instance_id.clone(),
            package: initialize.package_id.clone(),
            profile: initialize.profile_id.clone(),
            minor: initialize.selected_minor,
            features: initialize
                .enabled_features
                .iter()
                .filter_map(|value| provider::Feature::from_i32(*value))
                .collect(),
            authenticator: None,
        })
    }

    /// Tie the machine up. `BindAuthenticator`.
    ///
    /// ⚠️ This does not make the machine trusted: the descriptors and rights are checked by
    /// the broker, and the worker only knows to whom he was tied.
    pub fn bind(&mut self, bind: &provider::BindAuthenticator) -> Result<(), ContextError> {
        if self.authenticator.is_some()
            || carosign_protocol::validation::check_bind(bind).is_err()
            || bind.profile_id != self.profile
        {
            return Err(ContextError::MissingAuthenticator);
        }
        let authenticator = bind
            .authenticator
            .clone()
            .ok_or(ContextError::MissingAuthenticator)?;
        if carosign_protocol::validation::require_id(
            &authenticator.authenticator_id,
            "authenticator_id",
        )
        .is_err()
            || authenticator.device_epoch == 0
        {
            return Err(ContextError::MissingAuthenticator);
        }
        self.authenticator = Some(authenticator);
        Ok(())
    }

    /// Whether feature is included.
    pub fn has_feature(&self, feature: provider::Feature) -> bool {
        self.features.contains(&feature)
    }

    /// Is it tied to the machine?
    pub fn is_bound(&self) -> bool {
        self.authenticator.is_some()
    }
}
