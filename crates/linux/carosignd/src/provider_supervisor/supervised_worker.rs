use std::{io, process::Child, time::Duration};

use carosign_linux_ipc::provider_rendezvous::DiscoveredWorker;

pub(super) enum SupervisedWorker {
    Child(Child),
    Discovered(DiscoveredWorker),
}

impl SupervisedWorker {
    pub(super) fn has_exited(&mut self) -> io::Result<bool> {
        match self {
            Self::Child(child) => child.try_wait().map(|status| status.is_some()),
            Self::Discovered(worker) => worker.has_exited(),
        }
    }

    pub(super) fn stop(&mut self) -> io::Result<()> {
        if self.has_exited()? {
            return Ok(());
        }
        match match self {
            Self::Child(child) => child.kill(),
            Self::Discovered(worker) => worker.stop(),
        } {
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => Ok(()),
            result => result,
        }
    }

    pub(super) fn wait_exited(&mut self) -> io::Result<()> {
        match self {
            Self::Child(child) => child.wait().map(|_| ()),
            Self::Discovered(worker) => {
                if worker.wait_exited(Duration::from_secs(1))? {
                    Ok(())
                } else {
                    Err(io::ErrorKind::TimedOut.into())
                }
            }
        }
    }

    #[cfg(test)]
    pub(super) fn child_id(&self) -> u32 {
        let Self::Child(child) = self else {
            panic!("expected a child")
        };
        child.id()
    }
}
