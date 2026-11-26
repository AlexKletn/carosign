use carosign_protocol::carosign::{
    broker_relay::v1::Scope,
    common::v1::AuthenticatorRef,
    provider::v1::{
        AuthenticatorReady, Feature, Initialize, Initialized, PrebindDescribe, PrebindDescription,
        ProviderFrame, WorkerHello,
    },
};

use super::request_validation::valid_id;

#[derive(Default)]
pub(super) struct ProviderSetup {
    initialized: bool,
    profile: Option<Vec<u8>>,
    prebind_advertised: bool,
    prebind_enabled: bool,
    prebind_pending: Option<(Vec<u8>, AuthenticatorRef)>,
    prebind_finished: bool,
    ready: bool,
    transaction_advertised: bool,
    transaction_enabled: bool,
}

impl ProviderSetup {
    pub(super) fn advertise(&mut self, hello: &WorkerHello) {
        self.prebind_advertised = hello
            .features
            .contains(&(Feature::PrebindDescriptionV1 as i32));
        self.transaction_advertised = hello
            .features
            .contains(&(Feature::EnrollmentTransactionV1 as i32));
    }

    pub(super) fn begin_initialize(&mut self, init: &Initialize) -> Result<(), ()> {
        if self.initialized || self.profile.is_some() {
            return Err(());
        }
        let prebind_enabled = init
            .enabled_features
            .contains(&(Feature::PrebindDescriptionV1 as i32));
        let transaction_enabled = init
            .enabled_features
            .contains(&(Feature::EnrollmentTransactionV1 as i32));
        if (prebind_enabled && !self.prebind_advertised)
            || (transaction_enabled && !self.transaction_advertised)
        {
            return Err(());
        }
        self.prebind_enabled = prebind_enabled;
        self.transaction_enabled = transaction_enabled;
        self.profile = Some(init.profile_id.clone());
        Ok(())
    }

    pub(super) fn confirm_initialize(
        &mut self,
        frame: &ProviderFrame,
        initialized: &Initialized,
    ) -> Result<(), ()> {
        if self.initialized
            || self.profile.is_none()
            || !initialized.ready_for_binding
            || frame.device_epoch != 0
        {
            return Err(());
        }
        self.initialized = true;
        Ok(())
    }

    pub(super) fn request_description(
        &mut self,
        epoch: u64,
        frame: &ProviderFrame,
        request: &PrebindDescribe,
    ) -> Result<(), ()> {
        if !self.initialized
            || !self.prebind_enabled
            || self.ready
            || self.prebind_finished
            || self.prebind_pending.is_some()
        {
            return Err(());
        }
        let authenticator = request.authenticator.as_ref().ok_or(())?;
        if !valid_id(&authenticator.authenticator_id)
            || authenticator.device_epoch != epoch
            || !frame.operation_id.is_empty()
            || !valid_id(&frame.command_id)
        {
            return Err(());
        }
        self.prebind_pending = Some((frame.command_id.clone(), authenticator.clone()));
        Ok(())
    }

    pub(super) fn accept_description(
        &mut self,
        frame: &ProviderFrame,
        description: &PrebindDescription,
    ) -> Result<(), ()> {
        if !self.prebind_enabled || !self.initialized || self.prebind_finished {
            return Err(());
        }
        let (command, authenticator) = self.prebind_pending.take().ok_or(())?;
        if description.declaration_version != 1
            || description.authenticator.as_ref() != Some(&authenticator)
            || frame.command_id != command
            || !frame.operation_id.is_empty()
            || description.capabilities.is_none()
        {
            return Err(());
        }
        self.prebind_finished = true;
        Ok(())
    }

    pub(super) fn confirm_ready(
        &mut self,
        epoch: u64,
        scope: Option<&Scope>,
        value: &AuthenticatorReady,
    ) -> Result<(), ()> {
        if self.ready {
            return Err(());
        }
        let owner = scope.and_then(|scope| scope.owner.as_ref()).ok_or(())?;
        let authenticator = value.authenticator.as_ref().ok_or(())?;
        if authenticator.authenticator_id != owner.authenticator_id
            || authenticator.device_epoch != epoch
        {
            return Err(());
        }
        self.ready = true;
        Ok(())
    }

    pub(super) fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub(super) fn profile(&self) -> Option<&[u8]> {
        self.profile.as_deref()
    }

    pub(super) fn is_ready(&self) -> bool {
        self.ready
    }

    pub(super) fn transactions_enabled(&self) -> bool {
        self.transaction_enabled
    }
}
