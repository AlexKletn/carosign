use std::{
    collections::BTreeMap,
    os::fd::AsRawFd,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use carosign_core::ids::AuthenticatorId;
use carosign_runtime::ports::AuthorityRefusal;

use super::{QueueError, pending_device::PendingDevice, queue_state::QueueState};

pub(crate) struct DeviceQueue {
    limit: usize,
    ticket: AtomicU64,
    devices: Mutex<BTreeMap<AuthenticatorId, QueueState>>,
    changed: Condvar,
}

impl DeviceQueue {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            limit,
            ticket: AtomicU64::new(0),
            devices: Mutex::default(),
            changed: Condvar::new(),
        }
    }

    pub(crate) fn wait(
        &self,
        connection: &crate::server::Connection,
        id: AuthenticatorId,
        authorize: impl Fn() -> Result<(), AuthorityRefusal>,
    ) -> Result<PendingDevice<'_>, QueueError> {
        self.acquire(id, Instant::now() + Duration::from_secs(15), || {
            let mut fd = libc::pollfd {
                fd: connection.fd.as_raw_fd(),
                events: libc::POLLRDHUP,
                revents: 0,
            };
            // SAFETY: fd is owned by the connection and poll writes one local structure.
            let result = unsafe { libc::poll(&mut fd, 1, 0) };
            if result < 0
                || fd.revents & (libc::POLLRDHUP | libc::POLLHUP | libc::POLLERR | libc::POLLNVAL)
                    != 0
            {
                return Err(QueueError::Disconnected);
            }
            authorize().map_err(QueueError::Authority)
        })
    }

    fn acquire(
        &self,
        id: AuthenticatorId,
        deadline: Instant,
        check: impl Fn() -> Result<(), QueueError>,
    ) -> Result<PendingDevice<'_>, QueueError> {
        check()?;
        let mut devices = self.devices.lock().map_err(|_| QueueError::Unavailable)?;
        let device = devices.entry(id).or_default();
        if !device.active && device.waiting.is_empty() {
            device.active = true;
            return Ok(PendingDevice {
                queue: self,
                id,
                ticket: None,
            });
        }
        if device.waiting.len() >= self.limit {
            return Err(QueueError::Limit);
        }
        let ticket = self
            .ticket
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |ticket| {
                ticket.checked_add(1)
            })
            .map_err(|_| QueueError::Unavailable)?;
        device.waiting.push_back(ticket);
        drop(devices);
        let mut pending = PendingDevice {
            queue: self,
            id,
            ticket: Some(ticket),
        };
        loop {
            check()?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(QueueError::Timeout);
            }
            let mut devices = self.devices.lock().map_err(|_| QueueError::Unavailable)?;
            let device = devices.get_mut(&id).ok_or(QueueError::Unavailable)?;
            if !device.active && device.waiting.front() == Some(&ticket) {
                device.waiting.pop_front();
                device.active = true;
                pending.ticket = None;
                return Ok(pending);
            }
            drop(
                self.changed
                    .wait_timeout(devices, remaining.min(Duration::from_millis(20)))
                    .map_err(|_| QueueError::Unavailable)?,
            );
        }
    }

    pub(super) fn release(&self, id: AuthenticatorId, ticket: Option<u64>) {
        if let Ok(mut devices) = self.devices.lock()
            && let Some(device) = devices.get_mut(&id)
        {
            match ticket {
                Some(ticket) => device.waiting.retain(|queued| *queued != ticket),
                None => device.active = false,
            }
            if !device.active && device.waiting.is_empty() {
                devices.remove(&id);
            }
            self.changed.notify_all();
        }
    }
}

#[cfg(test)]
#[path = "device_queue.tests.rs"]
mod tests;
