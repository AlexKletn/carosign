use carosign_core::ids::AuthenticatorId;
use carosign_linux::sys;
use carosign_protocol::carosign::{
    common::v1::AuthenticatorDescriptor, provider_rendezvous::v1 as wire,
};
use prost::Message;
use std::{collections::BTreeSet, io, path::Path, time::Duration};

use super::{endpoint_identity::EndpointIdentity, publication::Publication};

pub(super) fn describe(
    publication: &Publication,
    root: &Path,
    timeout: Duration,
) -> io::Result<(Vec<AuthenticatorDescriptor>, EndpointIdentity)> {
    let endpoint = publication.endpoint(root)?;
    let metadata = std::fs::symlink_metadata(&endpoint)?;
    let fd = sys::connect_seqpacket(&endpoint)?;
    let peer =
        sys::peer_credentials(&fd).map_err(|error| io::Error::from_raw_os_error(error.errno))?;
    if (peer.uid, peer.gid) != (publication.uid, publication.gid) {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let id = sys::random_16().map_err(|error| io::Error::from_raw_os_error(error.errno))?;
    let request = wire::Request {
        version: 1,
        request_id: id.to_vec(),
        action: Some(wire::request::Action::Describe(wire::Describe {})),
    };
    let mut buffer = vec![0; carosign_protocol::limits::FRAME_MAX];
    let size = sys::exchange_seqpacket(&fd, &request.encode_to_vec(), &mut buffer, timeout)?;
    let identity = EndpointIdentity::of(&metadata, peer.pid);
    if identity
        != EndpointIdentity::of(
            &std::fs::symlink_metadata(publication.endpoint(root)?)?,
            peer.pid,
        )
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok((
        decode_inventory(&buffer[..size], &id, |id| {
            publication.devices.contains_key(&id)
        })?,
        identity,
    ))
}

fn decode_inventory(
    packet: &[u8],
    id: &[u8; 16],
    allowed: impl Fn(AuthenticatorId) -> bool,
) -> io::Result<Vec<AuthenticatorDescriptor>> {
    let response = wire::Response::decode(packet).map_err(|_| io::ErrorKind::InvalidData)?;
    if response.version != 1
        || response.request_id != id.as_slice()
        || response.encode_to_vec() != packet
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let Some(wire::response::Result::Inventory(inventory)) = response.result else {
        return Err(io::ErrorKind::InvalidData.into());
    };
    let mut seen = BTreeSet::new();
    let mut descriptors = inventory.authenticators;
    for descriptor in &mut descriptors {
        let authenticator = AuthenticatorId::try_from(descriptor.authenticator_id.as_slice())
            .map_err(|_| io::ErrorKind::InvalidData)?;
        if descriptor.unavailability_reason != 0
            || !allowed(authenticator)
            || !seen.insert(authenticator)
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        if let Some(capabilities) = &descriptor.capabilities {
            carosign_protocol::validation::check_declared_assurance(capabilities)
                .map_err(|_| io::ErrorKind::InvalidData)?;
        }
        descriptor.device_epoch = 0;
    }
    Ok(descriptors)
}

#[cfg(test)]
#[path = "endpoint.tests.rs"]
mod tests;
