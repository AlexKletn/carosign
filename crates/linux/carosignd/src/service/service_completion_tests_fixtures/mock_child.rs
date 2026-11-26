/// Only the test process: in case of panic and refusal does not leave the worker to live.
pub(super) struct MockChild {
    pub(super) child: std::process::Child,
    pub(super) pidfd: carosign_linux::sys::PidFd,
}

impl MockChild {
    pub(super) fn new(mut child: std::process::Child) -> std::io::Result<Self> {
        let pidfd = match carosign_linux::sys::PidFd::open(child.id()) {
            Ok(pidfd) => pidfd,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(std::io::Error::from_raw_os_error(error.errno));
            }
        };
        Ok(Self { child, pidfd })
    }

    pub(super) fn has_exited(&self) -> std::io::Result<bool> {
        self.pidfd
            .has_exited()
            .map_err(|error| std::io::Error::from_raw_os_error(error.errno))
    }

    /// Observation of the process does not confirm either the identity of the package or
    /// peace.
    pub(super) fn require_alive(&self) -> std::io::Result<()> {
        if self.has_exited()? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "mock-процесс завершился во время запуска",
            ));
        }
        Ok(())
    }

    pub(super) fn wait_bounded(
        &mut self,
        timeout: std::time::Duration,
    ) -> std::io::Result<std::process::ExitStatus> {
        let start = std::time::Instant::now();
        loop {
            if self.has_exited()?
                && let Some(status) = self.child.try_wait()?
            {
                return Ok(status);
            }
            if start.elapsed() >= timeout {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "процесс не завершился",
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

impl Drop for MockChild {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
