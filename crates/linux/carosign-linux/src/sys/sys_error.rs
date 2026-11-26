use core::fmt;

/// Core error.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SysError {
    /// The `errno` returned by the kernel.
    pub errno: i32,
}

impl fmt::Display for SysError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ядро отказало: errno {errno}", errno = self.errno)
    }
}

impl core::error::Error for SysError {}
