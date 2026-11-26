use super::super::gate::GateSender;
use super::super::gate::PolicyAdmissionError;
use super::OwnerState;
use super::WatchedPolicyAdmissions;
use crate::policy::PolicyChangeWatch;
use std::io;
use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;

/// Arranges the update of the picture and transfer of one-time permissions.
///
/// The caller provides the sole right to change the policy and verifies the authority of the
/// request.
pub(crate) struct PolicyOwner {
    inner: Mutex<OwnerState>,
}

impl PolicyOwner {
    pub(crate) fn new(
        generation: u64,
        watch: PolicyChangeWatch,
    ) -> Result<Self, PolicyAdmissionError> {
        Ok(Self {
            inner: Mutex::new(OwnerState {
                generation,
                active: Some(WatchedPolicyAdmissions::new(generation, watch)?),
            }),
        })
    }

    pub(crate) fn register(
        &self,
        generation: u64,
        sender: GateSender,
    ) -> Result<u64, PolicyAdmissionError> {
        let state = self
            .inner
            .lock()
            .map_err(|_| PolicyAdmissionError::Closed)?;
        state
            .active
            .as_ref()
            .ok_or(PolicyAdmissionError::Closed)?
            .register(generation, sender)
    }

    pub(crate) fn admit(&self, generation: u64, id: u64) -> Result<(), PolicyAdmissionError> {
        let state = self
            .inner
            .lock()
            .map_err(|_| PolicyAdmissionError::Closed)?;
        state
            .active
            .as_ref()
            .ok_or(PolicyAdmissionError::Closed)?
            .admit(generation, id)
    }

    /// First closes all the old gates, then updates and accepts only the new scanned image,
    /// and if any error is made, the owner remains closed; the repeat requires a new owner
    /// process.
    pub(crate) fn replace<F>(&self, update: F) -> Result<u64, PolicyAdmissionError>
    where
        F: FnOnce() -> std::io::Result<PolicyChangeWatch>,
    {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| PolicyAdmissionError::Closed)?;
        let active = state.active.take().ok_or(PolicyAdmissionError::Closed)?;
        active.revoke()?;
        let generation = state
            .generation
            .checked_add(1)
            .ok_or(PolicyAdmissionError::Limit)?;
        let watch = update().map_err(|_| PolicyAdmissionError::Io)?;
        let active = WatchedPolicyAdmissions::new(generation, watch)?;
        state.generation = generation;
        state.active = Some(active);
        Ok(generation)
    }
}

/// Accepts one update from a peer already vetted by the calling party.
/// The policy is recorded by the owner inside `replace`Peer verification, exclusive right to
/// write, and content verification are the responsibility of integration; until it is
/// available, this port is not a production service.
pub(crate) fn serve_installer_update<A, U>(
    owner: &PolicyOwner,
    mut stream: UnixStream,
    authorize: A,
    update: U,
) -> io::Result<()>
where
    A: FnOnce(&UnixStream) -> io::Result<bool>,
    U: FnOnce(&[u8]) -> io::Result<PolicyChangeWatch>,
{
    const MAX_UPDATE: usize = 1024 * 1024;
    if !authorize(&stream)? {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "installer denied",
        ));
    }
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > MAX_UPDATE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid update length",
        ));
    }
    let mut payload = vec![0; length];
    stream.read_exact(&mut payload)?;
    match owner.replace(|| update(&payload)) {
        Ok(generation) => {
            stream.write_all(&[0])?;
            stream.write_all(&generation.to_be_bytes())
        }
        Err(_) => {
            stream.write_all(&[1])?;
            Err(io::Error::other("policy update failed"))
        }
    }
}
