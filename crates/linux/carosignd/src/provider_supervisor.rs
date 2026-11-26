//! Run an broker and a single provider running a Linux service.
//!
//! The profile and executables are root-owned and protected from being written by other
//! users. SCM_RIGHTS It's not used.

mod cap_data;
mod cap_header;
mod identity;
mod profile;

mod supervised_worker;
use supervised_worker::SupervisedWorker;

use cap_data::CapData;
use cap_header::CapHeader;
pub use identity::ProviderIdentity;
pub use profile::ProviderProfile;
use profile::SystemResource;
pub(crate) use profile::parse_id;
#[cfg(test)]
use profile::{parse_extra_resource, protected_executable};

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
#[cfg(test)]
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use carosign_core::ids::IdSource;
use carosign_linux_ipc::provider::tunnel::TunnelClient;

fn pair() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [-1; 2];
    // SAFETY: The kernel fills two wholes with a successful socketpair.
    if unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            fds.as_mut_ptr(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: Both new descriptors belong to the caller.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

fn inherit(command: &mut Command, fds: Vec<i32>) {
    // SAFETY: In a child process before exec, only async-signal-safe fcntl is called; the
    // list contains valid socketpair descriptors.
    unsafe {
        command.pre_exec(move || {
            for &fd in &fds {
                let flags = libc::fcntl(fd, libc::F_GETFD);
                if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
}

fn restrict_worker(command: &mut Command) {
    // SAFETY: After fork, only system calls are used; before exec, the process loses all
    // ability and cannot get new ones through the file.
    unsafe {
        command.pre_exec(|| {
            let header = CapHeader {
                version: 0x20080522,
                pid: 0,
            };
            let data = [
                CapData {
                    effective: 0,
                    permitted: 0,
                    inheritable: 0,
                },
                CapData {
                    effective: 0,
                    permitted: 0,
                    inheritable: 0,
                },
            ];
            if libc::syscall(libc::SYS_capset, &header, data.as_ptr()) != 0
                || libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

pub(crate) fn hex(bytes: &[u8; 16]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(32);
    for byte in bytes {
        let _ = write!(result, "{byte:02x}");
    }
    result
}

/// Holds both processes and shuts them down when the demon stops.
pub struct ProviderSupervisor {
    modality: carosign_protocol::carosign::common::v1::Modality,
    worker: SupervisedWorker,
    broker: Child,
    resource_owner: Option<Child>,
    exclusive_resource_owner: bool,
    worker_id: carosign_core::ids::WorkerInstanceId,
    /// A private channel after a successful Hello with a mediator.
    tunnel: Option<TunnelClient>,
    /// Separate management of the broker does not share the reading of the events of
    /// the transaction.
    control: Option<OwnedFd>,
    /// Private endpoint of a trusted owner; FD itself is not a right.
    enrollment_endpoint: Option<OwnedFd>,
    enrollment_control_endpoint: Option<OwnedFd>,
    resource_fd: Option<SystemResource>,
    additional_resources: Vec<SystemResource>,
    /// A binding request could transfer resources even if there is no response yet.
    resource_binding_started: bool,
    durable_binding: bool,
}

impl ProviderSupervisor {
    #[cfg(test)]
    pub(crate) fn test_exclusive_owner(
        worker_id: carosign_core::ids::WorkerInstanceId,
        control: OwnedFd,
    ) -> Self {
        Self {
            modality: carosign_protocol::carosign::common::v1::Modality::Fingerprint,
            worker: SupervisedWorker::Child(Command::new("sleep").arg("60").spawn().unwrap()),
            broker: Command::new("sleep").arg("60").spawn().unwrap(),
            resource_owner: Some(Command::new("sleep").arg("60").spawn().unwrap()),
            exclusive_resource_owner: true,
            worker_id,
            tunnel: None,
            control: Some(control),
            enrollment_endpoint: None,
            enrollment_control_endpoint: None,
            resource_fd: None,
            additional_resources: Vec::new(),
            resource_binding_started: true,
            durable_binding: true,
        }
    }
    /// It starts processes and sets the channel. The error does not declare the launch
    /// successful.
    pub fn start(
        profile: &ProviderProfile,
        boot: [u8; 16],
        epoch: carosign_core::device::DeviceEpoch,
        timeout: Duration,
    ) -> io::Result<Self> {
        Self::start_mode(profile, boot, epoch, timeout, false)
    }

    /// An ad to Bind only runs a worker and broker with no resources and no owner.
    pub(crate) fn start_declaration(
        profile: &ProviderProfile,
        boot: [u8; 16],
        epoch: carosign_core::device::DeviceEpoch,
        timeout: Duration,
    ) -> io::Result<Self> {
        Self::start_mode(profile, boot, epoch, timeout, true)
    }

    fn start_mode(
        profile: &ProviderProfile,
        boot: [u8; 16],
        epoch: carosign_core::device::DeviceEpoch,
        timeout: Duration,
        declaration_only: bool,
    ) -> io::Result<Self> {
        let wire_epoch = epoch.value();
        let ids = carosign_linux::ids::KernelIds;
        let worker_id = ids
            .new_worker_instance_id()
            .map_err(|_| io::Error::other("нет ID"))?;
        let broker_id = *ids
            .new_connection_id()
            .map_err(|_| io::Error::other("нет ID"))?
            .as_bytes();
        let (daemon_fd, broker_fd) = pair()?;
        let owner_pair = if profile.resource_owner.is_some() && !declaration_only {
            Some(pair()?)
        } else {
            None
        };
        let control_pair = if owner_pair.is_some() {
            Some(pair()?)
        } else {
            None
        };
        let enrollment_pair =
            if owner_pair.is_some() && profile.enrollment_owner_endpoint && !declaration_only {
                Some(pair()?)
            } else {
                None
            };
        let enrollment_control_pair = if enrollment_pair.is_some() {
            Some(pair()?)
        } else {
            None
        };
        let owner_worker_pair =
            if owner_pair.is_some() && profile.owner_worker_endpoint && !declaration_only {
                Some(pair()?)
            } else {
                None
            };
        let (mut worker, broker_provider_fd) = if let Some(remote) = &profile.remote {
            let mut worker = remote.open(
                worker_id,
                epoch,
                declaration_only,
                timeout,
                owner_worker_pair.as_ref().map(|(worker, _)| worker),
            )?;
            let fd = worker.take_socket().ok_or(io::ErrorKind::BrokenPipe)?;
            (SupervisedWorker::Discovered(worker), fd)
        } else {
            start_local_worker(profile, declaration_only, owner_worker_pair.as_ref())?
        };
        let mut owner_child =
            if let (Some(path), Some((_, owner_fd))) = (&profile.resource_owner, &owner_pair) {
                let mut cmd = Command::new(path);
                cmd.args([
                    "--fd",
                    &owner_fd.as_raw_fd().to_string(),
                    "--boot",
                    &hex(&boot),
                    "--worker",
                    &hex(worker_id.as_bytes()),
                    "--authenticator",
                    &hex(profile.identity.authenticator.as_bytes()),
                    "--epoch",
                    &wire_epoch.to_string(),
                ])
                .env_clear()
                .stdin(Stdio::null());
                for (index, resource) in profile.owner_resources().enumerate() {
                    cmd.arg(format!("--resource-{index}")).arg(hex(&resource));
                }
                let mut owner_fds = vec![owner_fd.as_raw_fd()];
                if let Some((_, enrollment_fd)) = &enrollment_pair {
                    cmd.arg("--enrollment-fd")
                        .arg(enrollment_fd.as_raw_fd().to_string());
                    owner_fds.push(enrollment_fd.as_raw_fd());
                }
                if let Some((_, enrollment_control_fd)) = &enrollment_control_pair {
                    cmd.arg("--enrollment-control-fd")
                        .arg(enrollment_control_fd.as_raw_fd().to_string());
                    owner_fds.push(enrollment_control_fd.as_raw_fd());
                }
                if let Some((_, owner_fd)) = &owner_worker_pair {
                    cmd.arg("--worker-channel-fd")
                        .arg(owner_fd.as_raw_fd().to_string());
                    owner_fds.push(owner_fd.as_raw_fd());
                }
                inherit(&mut cmd, owner_fds);
                match cmd.spawn() {
                    Ok(child) => Some(child),
                    Err(error) => {
                        let _ = worker.stop();
                        let _ = worker.wait_exited();
                        return Err(error);
                    }
                }
            } else {
                None
            };
        let mut broker_cmd = Command::new(&profile.broker);
        broker_cmd
            .args([
                "--tunnel-fd",
                &broker_fd.as_raw_fd().to_string(),
                "--provider-fd",
                &broker_provider_fd.as_raw_fd().to_string(),
                "--worker",
                &hex(worker_id.as_bytes()),
                "--epoch",
                &wire_epoch.to_string(),
                "--broker",
                &hex(&broker_id),
                "--boot",
                &hex(&boot),
            ])
            .env_clear()
            .stdin(Stdio::null());
        if let Some((broker_owner_fd, _)) = &owner_pair {
            broker_cmd
                .arg("--owner-fd")
                .arg(broker_owner_fd.as_raw_fd().to_string());
        }
        if let Some((_, broker_control_fd)) = &control_pair {
            broker_cmd
                .arg("--control-fd")
                .arg(broker_control_fd.as_raw_fd().to_string());
        }
        if let Some(id) = profile.owner_managed_resource.filter(|_| !declaration_only) {
            broker_cmd.arg("--owner-managed-resource").arg(hex(&id));
        }
        if let Some(resource) = profile.resource_fd.as_ref().filter(|_| !declaration_only) {
            broker_cmd
                .arg("--resource-path")
                .arg(&resource.path)
                .arg("--resource-id")
                .arg(hex(&resource.id))
                .arg("--resource-role")
                .arg(match resource.role {
                    carosign_protocol::carosign::provider::v1::FdRole::Device => "device",
                    carosign_protocol::carosign::provider::v1::FdRole::Config => "config",
                    _ => unreachable!(),
                });
        }
        for resource in profile
            .additional_resources
            .iter()
            .filter(|_| !declaration_only)
        {
            broker_cmd
                .arg("--resource-path")
                .arg(&resource.path)
                .arg("--resource-id")
                .arg(hex(&resource.id))
                .arg("--resource-role")
                .arg(match resource.role {
                    carosign_protocol::carosign::provider::v1::FdRole::Device => "device",
                    carosign_protocol::carosign::provider::v1::FdRole::Config => "config",
                    _ => unreachable!(),
                });
        }
        let mut broker_fds = vec![broker_fd.as_raw_fd(), broker_provider_fd.as_raw_fd()];
        if let Some((broker_owner_fd, _)) = &owner_pair {
            broker_fds.push(broker_owner_fd.as_raw_fd());
        }
        if let Some((_, broker_control_fd)) = &control_pair {
            broker_fds.push(broker_control_fd.as_raw_fd());
        }
        if profile.remote.is_some() {
            broker_cmd.arg("--discovered-worker");
        }
        inherit(&mut broker_cmd, broker_fds);
        let mut broker = match broker_cmd.spawn() {
            Ok(child) => child,
            Err(error) => {
                if let Some(owner) = owner_child.as_mut() {
                    let _ = owner.kill();
                    let _ = owner.wait();
                }
                let _ = worker.stop();
                let _ = worker.wait_exited();
                return Err(error);
            }
        };
        drop((broker_fd, broker_provider_fd, owner_pair, owner_worker_pair));
        let control = control_pair.map(|(daemon, broker)| {
            drop(broker);
            daemon
        });
        let enrollment_endpoint = enrollment_pair.map(|(daemon, owner)| {
            drop(owner);
            daemon
        });
        let enrollment_control_endpoint = enrollment_control_pair.map(|(daemon, owner)| {
            drop(owner);
            daemon
        });
        match TunnelClient::connect(
            daemon_fd,
            *worker_id.as_bytes(),
            wire_epoch,
            broker_id,
            boot,
            timeout,
        ) {
            Ok(tunnel) => Ok(Self {
                modality: profile.identity().modality,
                worker,
                broker,
                resource_owner: owner_child,
                exclusive_resource_owner: profile.exclusive_resource_owner && !declaration_only,
                worker_id,
                tunnel: Some(tunnel),
                control,
                enrollment_endpoint,
                enrollment_control_endpoint,
                resource_fd: if declaration_only {
                    None
                } else {
                    profile.resource_fd.clone()
                },
                additional_resources: if declaration_only {
                    Vec::new()
                } else {
                    profile.additional_resources.clone()
                },
                resource_binding_started: false,
                durable_binding: !declaration_only && profile.requires_durable_binding(),
            }),
            Err(_) => {
                if let Some(owner) = owner_child.as_mut() {
                    let _ = owner.kill();
                    let _ = owner.wait();
                }
                let _ = broker.kill();
                let _ = broker.wait();
                let _ = worker.stop();
                let _ = worker.wait_exited();
                Err(io::Error::other("канал посредника недоступен"))
            }
        }
    }

    pub(crate) fn resource_slots_for_binding(&self) -> io::Result<Vec<([u8; 16], i32)>> {
        let mut slots = Vec::new();
        for resource in self.resource_fd.iter().chain(&self.additional_resources) {
            use carosign_protocol::carosign::provider::v1::FdRole;
            if resource.role != FdRole::Config
                && !(resource.role == FdRole::Device && self.has_exclusive_owner())
            {
                return Err(io::Error::other("нет устойчивого владельца ресурса"));
            }
            slots.push((resource.id, resource.role as i32));
        }
        Ok(slots)
    }

    /// It is noted before the request is transmitted to the broker, including the lost
    /// response.
    pub(crate) fn mark_resource_binding_started(&mut self) {
        self.resource_binding_started = true;
    }

    pub(crate) fn has_exclusive_owner(&self) -> bool {
        self.exclusive_resource_owner && self.resource_owner.is_some()
    }

    /// Only to a trusted lineup after the start of the Bind; the descriptor itself does not
    /// confirm the authority, scope, recall, or presence of the recipient.
    pub(crate) fn take_enrollment_endpoint(&mut self) -> Option<OwnedFd> {
        if !self.resource_binding_started || !self.has_exclusive_owner() {
            return None;
        }
        self.enrollment_endpoint.take()
    }

    pub(crate) fn take_enrollment_control_endpoint(&mut self) -> Option<OwnedFd> {
        if !self.resource_binding_started || !self.has_exclusive_owner() {
            return None;
        }
        self.enrollment_control_endpoint.take()
    }

    /// Transmits a separate disposable channel to the cleaning owner.
    pub(crate) fn take_control(&mut self) -> Option<crate::broker_control::BrokerControl> {
        self.control
            .take()
            .map(crate::broker_control::BrokerControl::new)
    }

    /// Closes the channel, completes both processes and confirms the cleaning to the owner.
    pub fn stop_and_confirm(
        &mut self,
        resources: &crate::resources::LocalResourceOwner,
    ) -> io::Result<()> {
        if let Some(tunnel) = self.tunnel.as_mut() {
            tunnel.abort();
        }
        let worker = self.worker.stop().and_then(|()| self.worker.wait_exited());
        let broker = stop_child(&mut self.broker);
        let owner = self.resource_owner.as_mut().map(stop_child).transpose();
        worker?;
        broker?;
        owner?;
        let cleanup = self.exit_cleanup();
        resources
            .confirm_exited(
                self.worker_id,
                self.worker.has_exited()?,
                self.broker.try_wait()?.is_some(),
                cleanup,
            )
            .map_err(|_| io::Error::other("завершение процессов не подтверждено"))
    }

    pub(crate) fn requires_durable_binding(&self) -> bool {
        self.durable_binding
    }

    pub(crate) fn exit_cleanup(&self) -> crate::resources::ExitCleanup {
        if self.resource_binding_started && self.durable_binding {
            crate::resources::ExitCleanup::HardwareUnconfirmed
        } else {
            crate::resources::ExitCleanup::ProcessOnly
        }
    }

    pub(crate) fn modality(&self) -> carosign_protocol::carosign::common::v1::Modality {
        self.modality
    }

    /// Worker Identifier to link the issue and the command.
    pub fn worker_id(&self) -> carosign_core::ids::WorkerInstanceId {
        self.worker_id
    }

    /// The same worker checks the actual existence of both processes.
    pub(crate) fn confirms_worker(&mut self, worker: carosign_core::ids::WorkerInstanceId) -> bool {
        worker == self.worker_id
            && self.worker.has_exited().is_ok_and(|exited| !exited)
            && self
                .broker
                .try_wait()
                .ok()
                .is_some_and(|status| status.is_none())
    }

    pub(crate) fn processes_stopped(
        &mut self,
        worker: carosign_core::ids::WorkerInstanceId,
    ) -> bool {
        worker == self.worker_id
            && self.worker.has_exited().is_ok_and(|exited| exited)
            && self
                .broker
                .try_wait()
                .ok()
                .is_some_and(|status| status.is_some())
    }

    /// Transfers the channel to the sole owner of the operation.
    pub fn take_tunnel(&mut self) -> Option<TunnelClient> {
        self.tunnel.take()
    }

    /// Returns the channel after completion of the operation; re-installation is prohibited.
    #[allow(clippy::result_large_err)] // If you refuse, return ownership of the previous channel.
    pub fn restore_tunnel(&mut self, tunnel: TunnelClient) -> Result<(), TunnelClient> {
        if self.tunnel.is_some() {
            return Err(tunnel);
        }
        self.tunnel = Some(tunnel);
        Ok(())
    }
}

fn stop_child(child: &mut Child) -> io::Result<()> {
    if child.try_wait()?.is_none() {
        match child.kill() {
            Err(error) if error.raw_os_error() != Some(libc::ESRCH) => return Err(error),
            _ => {}
        }
        child.wait()?;
    }
    Ok(())
}

fn start_local_worker(
    profile: &ProviderProfile,
    declaration_only: bool,
    owner_worker_pair: Option<&(std::os::fd::OwnedFd, std::os::fd::OwnedFd)>,
) -> io::Result<(SupervisedWorker, std::os::fd::OwnedFd)> {
    let (broker_provider_fd, provider_fd) = pair()?;
    let mut provider_cmd = Command::new(&profile.provider);
    provider_cmd
        .arg(provider_fd.as_raw_fd().to_string())
        .env_clear()
        .uid(profile.provider_uid)
        .gid(profile.provider_gid)
        .stdin(Stdio::null());
    restrict_worker(&mut provider_cmd);
    if declaration_only {
        provider_cmd.arg("--declare-only");
    }
    #[cfg(test)]
    if let Some(scenario) = &profile.mock_scenario {
        provider_cmd.arg(scenario);
    }
    #[cfg(test)]
    if let Some(path) = &profile.mock_state {
        provider_cmd.env("CAROSIGN_MOCK_STATE", path);
    }
    let mut provider_fds = vec![provider_fd.as_raw_fd()];
    if let Some((worker_fd, _)) = owner_worker_pair {
        provider_cmd
            .arg("--owner-channel-fd")
            .arg(worker_fd.as_raw_fd().to_string());
        provider_fds.push(worker_fd.as_raw_fd());
    }
    inherit(&mut provider_cmd, provider_fds);
    Ok((
        SupervisedWorker::Child(provider_cmd.spawn()?),
        broker_provider_fd,
    ))
}

impl Drop for ProviderSupervisor {
    fn drop(&mut self) {
        if let Some(tunnel) = self.tunnel.as_mut() {
            tunnel.abort();
        }
        let _ = self.broker.kill();
        let _ = self.worker.stop();
        if let Some(owner) = self.resource_owner.as_mut() {
            let _ = owner.kill();
        }
        let _ = self.broker.wait();
        let _ = self.worker.wait_exited();
        if let Some(owner) = self.resource_owner.as_mut() {
            let _ = owner.wait();
        }
    }
}

#[path = "provider_supervisor.tests.rs"]
#[cfg(test)]
mod tests;
