use carosign_core::ids::AuthenticatorId;
use carosign_protocol::carosign::common::v1::AuthenticatorDescriptor;
use std::{collections::BTreeMap, io};

use super::endpoint_identity::EndpointIdentity;

#[derive(Default)]
pub(super) struct InventoryEpochs {
    last: u64,
    known: BTreeMap<AuthenticatorId, AuthenticatorDescriptor>,
    current: BTreeMap<AuthenticatorId, (EndpointIdentity, u64)>,
}

impl InventoryEpochs {
    pub(super) fn invalidate_bindings(&mut self) {
        self.current.clear();
    }

    pub(super) fn binding(&self, id: AuthenticatorId) -> Option<(EndpointIdentity, u64)> {
        self.current.get(&id).copied()
    }

    pub(super) fn update(
        &mut self,
        entries: Vec<(AuthenticatorDescriptor, EndpointIdentity)>,
        allowed: impl IntoIterator<Item = AuthenticatorId>,
    ) -> io::Result<Vec<AuthenticatorDescriptor>> {
        let mut next = BTreeMap::new();
        let mut result = Vec::with_capacity(entries.len());
        for (mut descriptor, identity) in entries {
            let id = AuthenticatorId::try_from(descriptor.authenticator_id.as_slice())
                .map_err(|_| io::ErrorKind::InvalidData)?;
            descriptor.device_epoch = match self.current.get(&id) {
                Some((previous, epoch)) if *previous == identity => *epoch,
                _ => {
                    self.last = self
                        .last
                        .checked_add(1)
                        .ok_or_else(|| io::Error::other("предел эпох"))?;
                    self.last
                }
            };
            next.insert(id, (identity, descriptor.device_epoch));
            self.known.insert(id, descriptor.clone());
            result.push(descriptor);
        }
        let mut known = BTreeMap::new();
        for id in allowed {
            if let Some(mut descriptor) = self.known.remove(&id) {
                if !next.contains_key(&id) {
                    descriptor.available = false;
                    result.push(descriptor.clone());
                }
                known.insert(id, descriptor);
            }
        }
        self.known = known;
        self.current = next;
        Ok(result)
    }
}

#[cfg(test)]
#[path = "inventory_epochs.tests.rs"]
mod tests;
