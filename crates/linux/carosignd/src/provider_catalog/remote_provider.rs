use std::{
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use carosign_core::{
    device::{AuthenticatorRef, DeviceEpoch},
    ids::WorkerInstanceId,
};
use carosign_linux::{
    policy::{PolicyChangeWatch, PolicyWatchState},
    sys,
};
use carosign_linux_ipc::provider_rendezvous::{DiscoveredWorker, WorkerRequest};

use super::{
    endpoint_identity::EndpointIdentity, inventory_epochs::InventoryEpochs,
    publication::Publication,
};

pub(crate) struct RemoteProvider {
    pub(super) publication: Publication,
    pub(super) root: &'static Path,
    pub(super) runtime: PathBuf,
    pub(super) identity: EndpointIdentity,
    pub(super) authenticator: AuthenticatorRef,
    pub(super) epochs: Arc<Mutex<InventoryEpochs>>,
    pub(super) watches: Mutex<[PolicyChangeWatch; 2]>,
}

impl RemoteProvider {
    pub(crate) fn epoch(&self) -> DeviceEpoch {
        self.authenticator.device_epoch
    }

    pub(crate) fn open(
        &self,
        worker_id: WorkerInstanceId,
        epoch: DeviceEpoch,
        declaration_only: bool,
        timeout: Duration,
        owner_channel: Option<&std::os::fd::OwnedFd>,
    ) -> io::Result<DiscoveredWorker> {
        if epoch != self.epoch() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        self.require_current_binding()?;
        let endpoint = self.publication.endpoint(self.root)?;
        let control = sys::connect_seqpacket(&endpoint)?;
        let peer = sys::peer_credentials(&control)
            .map_err(|error| io::Error::from_raw_os_error(error.errno))?;
        let metadata = std::fs::symlink_metadata(self.publication.endpoint(self.root)?)?;
        let mut watches = self
            .watches
            .lock()
            .map_err(|_| io::ErrorKind::PermissionDenied)?;
        if EndpointIdentity::of(&metadata, peer.pid) != self.identity || !unchanged(&mut watches)? {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let request = WorkerRequest {
            authenticator: self.authenticator,
            worker_id,
            uid: self.publication.uid,
            gid: self.publication.gid,
            declaration_only,
        };
        let worker = match owner_channel {
            Some(channel) => {
                request.open_with_owner_channel(&control, &self.runtime, timeout, channel)?
            }
            None => request.open(&control, &self.runtime, timeout)?,
        };
        if !unchanged(&mut watches)?
            || EndpointIdentity::of(
                &std::fs::symlink_metadata(self.publication.endpoint(self.root)?)?,
                peer.pid,
            ) != self.identity
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        drop(watches);
        self.require_current_binding()?;
        Ok(worker)
    }

    pub(super) fn profile_is_current(&self) -> bool {
        self.watches
            .lock()
            .is_ok_and(|mut watches| unchanged(&mut watches).unwrap_or(false))
    }

    fn require_current_binding(&self) -> io::Result<()> {
        let epochs = self
            .epochs
            .lock()
            .map_err(|_| io::ErrorKind::PermissionDenied)?;
        if epochs.binding(self.authenticator.id) != Some((self.identity, self.epoch().value())) {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
    }
}

fn unchanged(watches: &mut [PolicyChangeWatch; 2]) -> io::Result<bool> {
    Ok(watches[0].check()? == PolicyWatchState::Unchanged
        && watches[1].check()? == PolicyWatchState::Unchanged)
}
