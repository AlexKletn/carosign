use super::super::signature_runtime;
use super::super::signature_status;
use super::super::verity;
use super::InputError;
use super::PinnedVerifier;
use super::SignatureCheckedArtifact;
use super::mount_attr::MountAttr;
use super::primary_version::primary_version;
use std::io;
use std::io::Write;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;
use std::time::Duration;

// Limits refer to the installer data, not the device contract.
const ARTIFACT_MAX: u64 = 256 * 1024 * 1024;

const SIGNATURE_MAX: u64 = 1024 * 1024;

pub(super) const KEYRING_MAX: u64 = 16 * 1024 * 1024;

/// Sealed copies; none is an installation permit.
pub(in crate::discovery) struct SignatureInputs {
    artifact: OwnedFd,
    signature: OwnedFd,
    keyring: OwnedFd,
    keyring_verity: Option<[u8; 32]>,
}

pub(super) fn snapshot(source: BorrowedFd<'_>, max: u64) -> Result<OwnedFd, InputError> {
    // SAFETY: stat is filled with the kernel; source is held by the caller.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(source.as_raw_fd(), &mut stat) } != 0 {
        return Err(InputError::Io(io::Error::last_os_error()));
    }
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG || stat.st_size < 0 {
        return Err(InputError::InvalidFile);
    }
    if stat.st_size as u64 > max {
        return Err(InputError::TooLarge);
    }
    // SAFETY: The name is a static string; we create our own anonymous file.
    let raw = unsafe {
        libc::memfd_create(
            c"carosign-signature-input".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    if raw < 0 {
        return Err(InputError::Io(io::Error::last_os_error()));
    }
    // SAFETY: memfd_create He handed over a new descriptor.
    let mut target = unsafe { std::fs::File::from_raw_fd(raw) };
    // SAFETY: The borrowed source is not transferred into possession: we duplicate it.
    let duplicate = unsafe { libc::fcntl(source.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    if duplicate < 0 {
        return Err(InputError::Io(io::Error::last_os_error()));
    }
    // SAFETY: fcntl returned a new descriptor.
    let input = unsafe { std::fs::File::from_raw_fd(duplicate) };
    let mut offset = 0u64;
    let mut buffer = [0u8; 16384];
    loop {
        let n = input.read_at(&mut buffer, offset)?;
        if n == 0 {
            break;
        }
        offset = offset.checked_add(n as u64).ok_or(InputError::TooLarge)?;
        if offset > max {
            return Err(InputError::TooLarge);
        }
        target.write_all(&buffer[..n])?;
    }
    // We prohibit writing and resizing, even if you save write FD.
    let seals = libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
    // SAFETY: Add the allowed seals to your own memfd.
    if unsafe { libc::fcntl(target.as_raw_fd(), libc::F_ADD_SEALS, seals) } < 0 {
        return Err(InputError::Io(io::Error::last_os_error()));
    }
    Ok(target.into())
}

impl SignatureInputs {
    /// Checks the signature in the new private mount namespace and the detached root.
    ///
    /// # Safety
    /// Only a single-threaded assistant with the right to create mount namespace.
    /// The independent owner must confirm the completeness of the runtime and the origin of
    /// the policy before the call; the result is not the authority of the installation.
    pub(in crate::discovery) unsafe fn verify_in_single_threaded_helper(
        self,
        verifier: &PinnedVerifier,
        runtime: &signature_runtime::PinnedRuntime,
        approved_primary: &str,
        timeout: Duration,
    ) -> Result<SignatureCheckedArtifact, InputError> {
        if !runtime.contains_verifier("usr/bin/gpgv", verifier.executable.as_fd())? {
            return Err(InputError::VerifierUnapproved);
        }
        // SAFETY: A single-threaded assistant only changes its namespace.
        if unsafe { libc::unshare(libc::CLONE_NEWNS | libc::CLONE_NEWNET) } != 0 {
            return Err(InputError::Io(io::Error::last_os_error()));
        }
        // We do not allow the distribution of mounts to other namespace.
        if unsafe {
            libc::mount(
                std::ptr::null(),
                c"/".as_ptr(),
                std::ptr::null(),
                libc::MS_REC | libc::MS_PRIVATE,
                std::ptr::null(),
            )
        } != 0
        {
            return Err(InputError::Io(io::Error::last_os_error()));
        }
        let root = crate::process::tmpfs_root::create(
            std::num::NonZeroU64::new(512 * 1024 * 1024).expect("ненулевой предел"),
        )
        .map_err(|error| InputError::Io(error.source))?;
        runtime
            .copy_into(root.as_fd())
            .map_err(InputError::Runtime)?;
        // memfd cannot be transferred open_treeWe copy the sealed images into private tmpfs,
        // then we edit them all for reading.
        if unsafe { libc::mkdirat(root.as_raw_fd(), c"inputs".as_ptr(), 0o700) } != 0 {
            return Err(InputError::Io(io::Error::last_os_error()));
        }
        let dir = unsafe {
            libc::openat(
                root.as_raw_fd(),
                c"inputs".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if dir < 0 {
            return Err(InputError::Io(io::Error::last_os_error()));
        }
        let dir = unsafe { OwnedFd::from_raw_fd(dir) };
        for (name, source) in [
            (c"artifact", self.artifact.as_fd()),
            (c"artifact.sig", self.signature.as_fd()),
            (c"trusted.gpg", self.keyring.as_fd()),
        ] {
            let fd = unsafe {
                libc::openat(
                    dir.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o400,
                )
            };
            if fd < 0 {
                return Err(InputError::Io(io::Error::last_os_error()));
            }
            let mut output = unsafe { std::fs::File::from_raw_fd(fd) };
            let duplicate = unsafe { libc::fcntl(source.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
            if duplicate < 0 {
                return Err(InputError::Io(io::Error::last_os_error()));
            }
            let input = unsafe { std::fs::File::from_raw_fd(duplicate) };
            let mut offset = 0;
            let mut buffer = [0u8; 16384];
            loop {
                let n = input.read_at(&mut buffer, offset)?;
                if n == 0 {
                    break;
                }
                output.write_all(&buffer[..n])?;
                offset += n as u64;
            }
        }
        let attr = MountAttr {
            attr_set: 1,
            attr_clr: 0,
            propagation: 0,
            userns_fd: 0,
        };
        // SAFETY: We only operate on the new mounting of tmpfs.
        if unsafe {
            libc::syscall(
                libc::SYS_mount_setattr,
                root.as_raw_fd(),
                c"".as_ptr(),
                libc::AT_EMPTY_PATH,
                &attr,
                std::mem::size_of::<MountAttr>(),
            )
        } != 0
        {
            return Err(InputError::Io(io::Error::last_os_error()));
        }
        // SAFETY: The root is built from files held in private namespace.
        unsafe {
            signature_status::verify_in_isolated_root(root.as_fd(), approved_primary, timeout)
        }
        .map_err(InputError::Signature)?;
        let primary_key_version = primary_version(self.keyring.as_fd(), approved_primary)?;
        Ok(SignatureCheckedArtifact {
            artifact: self.artifact,
            signature: self.signature,
            primary_fingerprint: approved_primary.to_owned(),
            primary_key_version,
            keyring_verity: self.keyring_verity,
            verifier_verity: verifier.verity,
        })
    }

    #[cfg(test)]
    pub(in crate::discovery) fn fixture(
        artifact: BorrowedFd<'_>,
        signature: BorrowedFd<'_>,
        keyring: BorrowedFd<'_>,
    ) -> Result<Self, InputError> {
        Ok(Self {
            artifact: snapshot(artifact, ARTIFACT_MAX)?,
            signature: snapshot(signature, SIGNATURE_MAX)?,
            keyring: snapshot(keyring, KEYRING_MAX)?,
            keyring_verity: None,
        })
    }

    /// The expected measurement must come from an approved system policy.
    /// An ordinary keyset file or measurement mismatch is a failure.
    pub(in crate::discovery) fn prepare(
        artifact: BorrowedFd<'_>,
        signature: BorrowedFd<'_>,
        keyring: BorrowedFd<'_>,
        approved_keyring_verity: &[u8; 32],
    ) -> Result<Self, InputError> {
        if !verity::matches_approved_digest(keyring, approved_keyring_verity)? {
            return Err(InputError::KeyringUnapproved);
        }
        let keyring = snapshot(keyring, KEYRING_MAX)?;
        Ok(Self {
            artifact: snapshot(artifact, ARTIFACT_MAX)?,
            signature: snapshot(signature, SIGNATURE_MAX)?,
            keyring,
            keyring_verity: Some(*approved_keyring_verity),
        })
    }

    /// Returns only the result of verification of the signature, not the authority to
    /// install.
    ///
    /// # Safety
    /// Called only by a single-threaded assistant, the composition must ensure the origin of
    /// the verification program and its surroundings before being called.
    #[cfg(test)]
    pub(in crate::discovery) unsafe fn verify_unisolated_fixture(
        self,
        verifier: &PinnedVerifier,
        approved_primary: &str,
        timeout: Duration,
    ) -> Result<SignatureCheckedArtifact, InputError> {
        // SAFETY: The single-threaded call and environment contract is transferred to the
        // caller.
        unsafe {
            signature_status::verify_in_single_threaded_helper(
                verifier.executable.as_fd(),
                self.artifact.as_fd(),
                self.signature.as_fd(),
                self.keyring.as_fd(),
                approved_primary,
                timeout,
            )
        }
        .map_err(InputError::Signature)?;
        let primary_key_version = primary_version(self.keyring.as_fd(), approved_primary)?;
        Ok(SignatureCheckedArtifact {
            artifact: self.artifact,
            signature: self.signature,
            primary_fingerprint: approved_primary.to_owned(),
            primary_key_version,
            keyring_verity: self.keyring_verity,
            verifier_verity: verifier.verity,
        })
    }
}
