//! Accept cancellation on the connection while the performer is occupied by the provider.

use carosign_core::ids::OperationId;
use carosign_linux_ipc::codec::ClientRequest;
use carosign_protocol::carosign::client::v1 as wire;
use prost::Message;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

mod cancellation_target;
pub(crate) use cancellation_target::CancellationTarget;

pub(crate) struct CancelReader {
    stop: Arc<AtomicBool>,
    _target: Option<Arc<CancellationTarget>>,
    handling: Arc<AtomicBool>,
    probe: OwnedFd,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CancelReader {
    pub(crate) fn start(
        connection: &crate::server::Connection,
        operation: OperationId,
        registry: &crate::operations::OperationRegistry,
        sender: crate::control_watch::RevocationSender,
        control: Arc<crate::control_watch::ControlOwner>,
        cleanup: Arc<crate::resources::SupervisorCleanup>,
        rollback_requested: Option<Arc<AtomicBool>>,
    ) -> std::io::Result<Self> {
        let fd = connection.fd.try_clone()?;
        let probe = connection.fd.try_clone()?;
        let version = connection
            .session
            .negotiated()
            .ok_or_else(|| std::io::Error::other("нет версии"))?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let handling = Arc::new(AtomicBool::new(false));
        let processing = handling.clone();
        let target = Arc::new(CancellationTarget {
            sender: sender.clone(),
            rollback: rollback_requested.clone(),
            handling: handling.clone(),
            stopped: stop.clone(),
            requested: std::sync::Mutex::new(false),
        });
        let local = target.clone();
        let thread = std::thread::Builder::new()
            .name("carosign-cancel-reader".into())
            .spawn(move || {
                let mut bytes = vec![0u8; carosign_protocol::limits::FRAME_MAX];
                while !stopped.load(Ordering::Acquire) {
                    if control.ready(operation) {
                        // The deadline has expired or the policy has changed: wake up the
                        // owner of the operation, waiting for the worker’s response.
                        let _ = cleanup.interrupt();
                        break;
                    }
                    let mut poll = libc::pollfd {
                        fd: fd.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    // SAFETY: pollfd and the stream descriptor are valid.
                    let ready = unsafe { libc::poll(&mut poll, 1, 20) };
                    if ready < 0 {
                        processing.store(true, Ordering::Release);
                        let _ = sender.cancel_and_wait(Duration::from_secs(2));
                        let _ = cleanup.interrupt();
                        break;
                    }
                    if ready == 0 {
                        continue;
                    }
                    // Just look at other people's requests: they should read the talk.
                    let size = unsafe {
                        libc::recv(
                            fd.as_raw_fd(),
                            bytes.as_mut_ptr().cast(),
                            bytes.len(),
                            libc::MSG_PEEK | libc::MSG_DONTWAIT,
                        )
                    };
                    if size == 0 {
                        processing.store(true, Ordering::Release);
                        // The client will no longer receive the result: we stop the operation
                        // and release the resource after the usual cleaning.
                        let _ = sender.cancel_and_wait(Duration::from_secs(2));
                        let _ = cleanup.interrupt();
                        break;
                    }
                    if size < 0 {
                        if std::io::Error::last_os_error().kind() == std::io::ErrorKind::WouldBlock
                        {
                            continue;
                        }
                        processing.store(true, Ordering::Release);
                        let _ = sender.cancel_and_wait(Duration::from_secs(2));
                        let _ = cleanup.interrupt();
                        break;
                    }
                    let Ok((
                        frame,
                        ClientRequest::CancelOperation {
                            operation: requested,
                        },
                    )) = carosign_linux_ipc::codec::decode(&bytes[..size as usize])
                    else {
                        std::thread::sleep(Duration::from_millis(20));
                        continue;
                    };
                    if frame.major != version.major
                        || frame.minor != version.minor
                        || requested != operation.as_bytes()
                    {
                        std::thread::sleep(Duration::from_millis(20));
                        continue;
                    }
                    // Before consume, we set a sign: check before commit should not miss the
                    // window between reading and review.
                    processing.store(true, Ordering::Release);
                    if consume(&fd, &mut bytes).is_err() {
                        break;
                    }
                    if local.request().is_err() {
                        break;
                    }
                    let reply = wire::ClientFrame {
                        major: version.major,
                        minor: version.minor,
                        request_id: frame.request_id,
                        operation_id: Vec::new(),
                        event_sequence: 0,
                        payload: Some(wire::client_frame::Payload::CancellationReceipt(
                            wire::CancellationReceipt {
                                result: wire::CancelReceiptKind::CancelReceiptRequested as i32,
                            },
                        )),
                    }
                    .encode_to_vec();
                    // SAFETY: The buffer and descriptor are alive during non-blocking
                    // shipment.
                    let sent = unsafe {
                        libc::send(
                            fd.as_raw_fd(),
                            reply.as_ptr().cast(),
                            reply.len(),
                            libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                        )
                    };
                    if sent != reply.len() as isize {
                        break;
                    }
                    // Interrupting the worker's blocking reading, the issuance is withheld
                    // for now; it's only released by a regular cleaning confirmation.
                    if rollback_requested.is_none() {
                        let _ = cleanup.interrupt();
                    }
                    break;
                }
            })?;
        let reader = Self {
            stop,
            _target: Some(target.clone()),
            handling,
            probe,
            thread: Some(thread),
        };
        registry
            .register_cancellation(operation, Arc::downgrade(&target))
            .ok_or_else(|| std::io::Error::other("нет операции отмены"))?;
        Ok(reader)
    }

    /// A picture of the irreversible border. MSG_PEEK: reading flow is the only consumer.
    /// Mistakes and breakups prevent success.
    pub(crate) fn pending_cancel_or_closed(&self) -> bool {
        if self.handling.load(Ordering::Acquire) {
            return true;
        }
        let mut poll = libc::pollfd {
            fd: self.probe.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut poll, 1, 0) } < 0
            || poll.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
        {
            return true;
        }
        let mut bytes = vec![0u8; carosign_protocol::limits::FRAME_MAX];
        let size = unsafe {
            libc::recv(
                self.probe.as_raw_fd(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                libc::MSG_PEEK | libc::MSG_DONTWAIT,
            )
        };
        let pending = if size == 0 {
            true
        } else if size < 0 {
            std::io::Error::last_os_error().kind() != std::io::ErrorKind::WouldBlock
        } else {
            // For any request in the queue can already be canceled.
            // The only reader is still looking at it.
            true
        };
        pending || self.handling.load(Ordering::Acquire)
    }
}

fn consume(fd: &OwnedFd, bytes: &mut [u8]) -> Result<(), ()> {
    let mut iov = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    // SAFETY: Empty control rejects transmitted descriptors through CTRUNC.
    let mut msg: libc::msghdr = unsafe { core::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    let size = unsafe {
        libc::recvmsg(
            fd.as_raw_fd(),
            &mut msg,
            libc::MSG_CMSG_CLOEXEC | libc::MSG_DONTWAIT,
        )
    };
    if size <= 0 || msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 {
        return Err(());
    }
    carosign_linux_ipc::frame::check(&bytes[..size as usize], false).map_err(|_| ())
}

impl Drop for CancelReader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[path = "cancel_reader.tests.rs"]
#[cfg(test)]
mod boundary_tests;
