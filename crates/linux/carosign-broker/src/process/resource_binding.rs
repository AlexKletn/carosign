use std::os::fd::OwnedFd;
use std::time::Duration;

use carosign_protocol::carosign::{
    broker_relay::v1::{AccessMode, Bind, Scope},
    provider::v1::{FdRole, ProviderFrame, provider_frame::Payload},
};

use crate::{owner_channel::OwnerSession, transport::TransportError};

use super::{
    ConfiguredResource, Context,
    configured_resource::{open_resource, validate_owned_resource},
    request_validation::{checked_scope, valid_id},
};

pub(super) struct ResourceBinding {
    configured: Vec<ConfiguredResource>,
    owner_channel: Option<OwnedFd>,
    owner_managed: Option<[u8; 16]>,
    pub(super) owner_session: Option<OwnerSession>,
}

impl ResourceBinding {
    pub(super) fn dispatch_to_owner(
        &mut self,
        request_id: [u8; 16],
        frame: &ProviderFrame,
    ) -> Result<(), ()> {
        if self.owner_managed.is_some() {
            self.owner_session.as_mut().ok_or(())?
                .dispatch(request_id, frame, Duration::from_secs(5))
                .map_err(|_| ())?;
        }
        Ok(())
    }

    pub(super) fn new(
        resources: Vec<ConfiguredResource>,
        owner_channel: Option<OwnedFd>,
        owner_managed: Option<[u8; 16]>,
    ) -> Result<Self, TransportError> {
        if owner_managed.is_some_and(|id| {
            id == [0; 16]
                || owner_channel.is_none()
                || resources.iter().any(|resource| {
                    resource.resource_id == id || resource.role != FdRole::Config as i32
                })
        }) || resources.len() > carosign_protocol::limits::FD_SLOTS_MAX
            || resources.iter().enumerate().any(|(index, entry)| {
                entry.resource_id == [0; 16]
                    || resources[..index]
                        .iter()
                        .any(|earlier| earlier.resource_id == entry.resource_id)
                    || !matches!(
                        FdRole::from_i32(entry.role),
                        Some(FdRole::Device | FdRole::Config)
                    )
            })
        {
            return Err(TransportError::Invalid);
        }

        Ok(Self {
            configured: resources,
            owner_channel,
            owner_managed,
            owner_session: None,
        })
    }

    pub(super) fn allows_prebind(&self) -> bool {
        self.configured.is_empty() && self.owner_channel.is_none()
    }

    pub(super) fn bind(
        &mut self,
        context: &Context,
        initialized_profile: Option<&[u8]>,
        expected: [u8; 16],
        bind: Bind,
    ) -> Result<(Scope, ProviderFrame, Vec<OwnedFd>), ()> {
        let mut bind_fds = Vec::new();
        let given = bind.scope.ok_or(())?;
        let provider = bind.frame.ok_or(())?;
        let Some(Payload::BindAuthenticator(body)) = provider.payload.as_ref() else {
            return Err(());
        };
        let owner = given.owner.as_ref().ok_or(())?;
        let authenticator = body.authenticator.as_ref().ok_or(())?;
        if !checked_scope(&given, context)
            || body.lease_id != given.lease_id
            || !valid_id(&body.profile_id)
            || initialized_profile != Some(body.profile_id.as_slice())
            || provider.operation_id != owner.operation_id
            || authenticator.authenticator_id != owner.authenticator_id
            || authenticator.device_epoch != context.epoch
        {
            return Err(());
        }
        if let Some(id) = self.owner_managed
            && (given.claims.len() != self.configured.len() + 1
                || !given.claims.iter().any(|claim| {
                    claim.resource_id == id && claim.mode == AccessMode::Exclusive as i32
                }))
        {
            return Err(());
        }
        if bind.slots.len() != self.configured.len() || body.fd_slots.len() != self.configured.len()
        {
            return Err(());
        }
        for (index, configured) in self.configured.iter().enumerate() {
            let slot = &bind.slots[index];
            let provider_slot = &body.fd_slots[index];
            if slot.resource_id != configured.resource_id
                || slot.role != configured.role
                || provider_slot.index as usize != index
                || provider_slot.role != configured.role
                || provider_slot.resource_id != configured.resource_id
                || !given.claims.iter().any(|claim| {
                    claim.resource_id == configured.resource_id
                        && claim.mode == AccessMode::Exclusive as i32
                })
            {
                return Err(());
            }
        }
        if let Some(channel) = self.owner_channel.take() {
            let has_device = self
                .configured
                .iter()
                .any(|resource| resource.role == FdRole::Device as i32);
            let owner_supplies_slot = |role| {
                self.owner_managed.is_none() && (!has_device || role == FdRole::Device as i32)
            };
            let request = carosign_protocol::carosign::resource_owner::v1::Prepare {
                request_id: expected.to_vec(),
                boot_id: context.boot.to_vec(),
                scope: Some(given.clone()),
                slots: bind
                    .slots
                    .iter()
                    .filter(|slot| owner_supplies_slot(slot.role))
                    .cloned()
                    .collect(),
            };
            match crate::owner_channel::prepare_once(channel, &request, Duration::from_secs(10))
                .map_err(|_| ())?
            {
                crate::owner_channel::Reply::Prepared {
                    channel,
                    descriptors,
                } => {
                    if descriptors.len() != request.slots.len() {
                        return Err(());
                    }
                    let mut descriptors = descriptors.into_iter();
                    for resource in &self.configured {
                        if owner_supplies_slot(resource.role) {
                            let fd = descriptors.next().ok_or(())?;
                            validate_owned_resource(resource, &fd).map_err(|_| ())?;
                            if resource.role == FdRole::Config as i32 {
                                bind_fds.push(open_resource(resource).map_err(|_| ())?);
                            } else {
                                bind_fds.push(fd);
                            }
                        } else {
                            bind_fds.push(open_resource(resource).map_err(|_| ())?);
                        }
                    }
                    self.owner_session = Some(channel);
                }
                crate::owner_channel::Reply::Refused(_) => return Err(()),
            }
        } else {
            for configured in &self.configured {
                bind_fds.push(open_resource(configured).map_err(|error| {
                    eprintln!("carosign-broker: ресурс недоступен: {error}");
                })?);
            }
        }
        self.configured.clear();
        Ok((given, provider, bind_fds))
    }
}

#[cfg(test)]
#[path = "resource_binding.tests.rs"]
mod tests;
