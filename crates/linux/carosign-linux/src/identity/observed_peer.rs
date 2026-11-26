use super::SystemdObservation;
use crate::sys::PeerUser;
use crate::sys::PidFd;
use crate::sys::SysError;
use crate::sys::peer_credentials;
use std::os::fd::AsFd;
use std::os::fd::FromRawFd;
use std::os::fd::IntoRawFd;
use std::os::fd::OwnedFd;

#[allow(dead_code)]
pub(crate) struct ObservedPeer {
    socket: OwnedFd,
    pidfd: PidFd,
    credentials: PeerUser,
}

#[allow(dead_code)]
impl ObservedPeer {
    pub(crate) fn capture(socket: OwnedFd) -> Result<Self, SysError> {
        let credentials = peer_credentials(&socket)?;
        let pidfd = PidFd::from_socket_peer(&socket)?;
        if pidfd.has_exited()? {
            return Err(SysError { errno: libc::ESRCH });
        }
        Ok(Self {
            socket,
            pidfd,
            credentials,
        })
    }

    pub(crate) fn credentials(&self) -> PeerUser {
        self.credentials
    }

    pub(crate) fn has_exited(&self) -> Result<bool, SysError> {
        self.pidfd.has_exited()
    }

    pub(crate) fn socket(&self) -> &OwnedFd {
        &self.socket
    }

    /// Verifies a live peer executable file with a pre-approved measurement.
    /// This is an observation, not an authority: the origin of the measurement, the bus and
    /// the unit are checked separately.
    pub(crate) fn matches_executable_verity(
        &self,
        expected_uid: u32,
        expected_digest: &[u8; 32],
    ) -> Result<(), ()> {
        if self.credentials.uid != expected_uid
            || *expected_digest == [0; 32]
            || self.has_exited().map_err(|_| ())?
        {
            return Err(());
        }
        // The numerical PID is valid only as a name in procfs: the retained pidfd is checked
        // before and after both openings. There is no spare pidfd over the PID.
        let path = format!("/proc/{}/exe", self.credentials.pid);
        let first = crate::sys::open_read(&path).map_err(|_| ())?;
        let metadata = crate::sys::launch_metadata(&first).map_err(|_| ())?;
        if metadata.mode & libc::S_IFMT != libc::S_IFREG
            || !crate::discovery::verity::matches_approved_digest(first.as_fd(), expected_digest)
                .map_err(|_| ())?
        {
            return Err(());
        }
        let second = crate::sys::open_read(&path).map_err(|_| ())?;
        if crate::sys::launch_metadata(&second).map_err(|_| ())? != metadata
            || self.has_exited().map_err(|_| ())?
        {
            return Err(());
        }
        Ok(())
    }

    /// Observation of manager response, not confirmed identity: the caller must separately
    /// establish trust in the bus, unit and its installation.
    fn observe_systemd_instance(
        &self,
        bus: &dbus::blocking::Connection,
    ) -> Result<SystemdObservation, ()> {
        if self.has_exited().map_err(|_| ())? {
            return Err(());
        }
        // Holding the owner's unique manager name: restarting or replacing the owner between
        // requests makes surveillance unusable.
        // This does not establish confidence in the tire itself.
        let daemon = bus.with_proxy(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            std::time::Duration::from_secs(3),
        );
        let owner = || -> Result<String, ()> {
            let (name,): (String,) = daemon
                .method_call(
                    "org.freedesktop.DBus",
                    "GetNameOwner",
                    ("org.freedesktop.systemd1",),
                )
                .map_err(|_| ())?;
            if !name.starts_with(':') {
                return Err(());
            }
            Ok(name)
        };
        let first_owner = owner()?;
        let fd = self.pidfd.duplicate().map_err(|_| ())?;
        // SAFETY: The copy belongs to us; the ownership goes to D-Bus.
        let fd = unsafe { dbus::arg::OwnedFd::from_raw_fd(fd.into_raw_fd()) };
        let proxy = bus.with_proxy(
            first_owner.as_str(),
            "/org/freedesktop/systemd1",
            std::time::Duration::from_secs(3),
        );
        let (_path, unit, invocation): (dbus::Path<'static>, String, Vec<u8>) = proxy
            .method_call("org.freedesktop.systemd1.Manager", "GetUnitByPIDFD", (fd,))
            .map_err(|_| ())?;
        let invocation: [u8; 16] = invocation.try_into().map_err(|_| ())?;
        if unit.is_empty()
            || invocation == [0; 16]
            || owner()? != first_owner
            || self.has_exited().map_err(|_| ())?
        {
            return Err(());
        }
        Ok(SystemdObservation {
            owner: first_owner,
            unit,
            invocation,
        })
    }

    /// Returns surveillance without asserting trust in the source.
    pub(crate) fn observe_systemd_unit(
        &self,
        bus: &dbus::blocking::Connection,
    ) -> Result<(String, [u8; 16]), ()> {
        let observation = self.observe_systemd_instance(bus)?;
        Ok((observation.unit, observation.invocation))
    }

    /// Verifies two observations of one retained peer with the expected unit and its specific
    /// launch. Trust in the bus and the origin of the expectation are not established here;
    /// the caller must confirm them separately.
    pub(crate) fn matches_systemd_invocation(
        &self,
        bus: &dbus::blocking::Connection,
        expected_unit: &str,
        expected_invocation: [u8; 16],
    ) -> Result<(), ()> {
        if expected_unit.is_empty() || expected_invocation == [0; 16] {
            return Err(());
        }
        let first = self.observe_systemd_instance(bus)?;
        let second = self.observe_systemd_instance(bus)?;
        if !matching_invocation(&first, &second, expected_unit, expected_invocation)
            || self.has_exited().map_err(|_| ())?
        {
            return Err(());
        }
        Ok(())
    }
}

pub(super) fn matching_invocation(
    first: &SystemdObservation,
    second: &SystemdObservation,
    expected_unit: &str,
    expected_invocation: [u8; 16],
) -> bool {
    !expected_unit.is_empty()
        && expected_invocation != [0; 16]
        && first == second
        && first.owner.starts_with(':')
        && first.unit == expected_unit
        && first.invocation == expected_invocation
}
