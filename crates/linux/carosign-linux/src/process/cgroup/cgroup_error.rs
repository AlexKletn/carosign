use std::io;

/// Rejection should not be interpreted as permission to run a process without cgroup.
#[derive(Debug)]
pub(crate) enum CgroupError {
    InvalidName,
    InvalidCpuPeriod,
    WrongFilesystem,
    Kernel {
        stage: &'static str,
        source: io::Error,
    },
    ShortWrite {
        stage: &'static str,
    },
    Gate(io::Error),
}
