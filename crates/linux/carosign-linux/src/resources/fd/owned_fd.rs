use core::fmt;

/// A description that the platform has discovered itself.
///
/// ⚠️ Lives as a meaning: `fd` It is a number, but it is not “right” in itself.
/// The right is to extradite an broker.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct OwnedFd {
    fd: i32,
}

impl OwnedFd {
    /// Takes the numerical descriptor.
    ///
    /// ⚠️ The caller transfers possession: the closure is on us.
    pub fn from_raw(fd: i32) -> Option<Self> {
        if fd < 0 { None } else { Some(Self { fd }) }
    }

    /// Numerical view (for `libc`- Challenges.
    pub fn raw(&self) -> i32 {
        self.fd
    }

    /// It is only a matter of forgetting the number, but it is a matter of `close` binds to
    /// the core.
    pub fn release(self) -> i32 {
        self.fd
    }
}

impl fmt::Debug for OwnedFd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // ⚠️ We do not write a number in the magazine: you can pull someone else's resource
        // on it.
        f.write_str("OwnedFd(..)")
    }
}
