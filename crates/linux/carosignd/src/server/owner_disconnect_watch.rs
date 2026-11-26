/// Watches the owner close while the handler is busy outside of reading the socket.
/// The observer does not read the footage and does not establish identity or authority.
pub(crate) struct OwnerDisconnectWatch {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl OwnerDisconnectWatch {
    pub(crate) fn start(
        fd: &std::os::fd::OwnedFd,
        registry: std::sync::Arc<crate::operations::OperationRegistry>,
        handle: crate::operations::ConnectionHandle,
    ) -> std::io::Result<Self> {
        use std::os::fd::AsRawFd;
        use std::sync::atomic::Ordering;
        let fd = fd.try_clone()?;
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopped = stop.clone();
        let thread = std::thread::Builder::new()
            .name("carosign-owner-watch".into())
            .spawn(move || {
                let observation = std::panic::catch_unwind(|| {
                    let mut poll = libc::pollfd {
                        fd: fd.as_raw_fd(),
                        events: libc::POLLRDHUP,
                        revents: 0,
                    };
                    while !stopped.load(Ordering::Acquire) {
                        // SAFETY: pollfd and the stream-owned descriptor operate.
                        let result = unsafe { libc::poll(&mut poll, 1, 20) };
                        if result < 0 {
                            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                                continue;
                            }
                            return true;
                        }
                        if poll.revents
                            & (libc::POLLRDHUP | libc::POLLHUP | libc::POLLERR | libc::POLLNVAL)
                            != 0
                        {
                            return true;
                        }
                    }
                    false
                });
                // The loss of an observer does not preserve the right of the owner to
                // respond.
                if !matches!(observation, Ok(false)) {
                    eprintln!("carosignd: соединение клиента закрыто во время обработки запроса");
                    registry.detach_connection(handle);
                }
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for OwnerDisconnectWatch {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
