use std::os::fd::AsRawFd;

use super::*;
use std::io::Write;
use std::os::fd::AsFd;

#[test]
fn enabled_verity_measures_and_refuses_changes() {
    let path = std::env::temp_dir().join(format!(
        "carosign-enable-verity-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    // We only delete the file created by the test, even in panic.
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let mut writer = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    let _cleanup = Cleanup(path.clone());
    writer.write_all(b"biometric fs-verity fixture\n").unwrap();
    writer.sync_all().unwrap();
    // You can't turn on fs-verity while the file is open for recording.
    drop(writer);
    let file = std::fs::File::open(&path).unwrap();
    let measured = match enable_and_measure(file.as_fd()) {
        Ok(digest) => digest,
        Err(error) => {
            let unsupported = matches!(
                error.raw_os_error(),
                Some(libc::ENOTTY | libc::EOPNOTSUPP | libc::ENOSYS | libc::EINVAL)
            );
            if !unsupported || std::env::var_os("CAROSIGN_VERITY_REQUIRE_SUCCESS").is_some() {
                panic!("fs-verity недоступен: {error}");
            }
            eprintln!("fs-verity недоступен на временной ФС: {error}");
            return;
        }
    };
    let mut digest = Digest {
        algorithm: 0,
        size: 32,
        bytes: [0; 32],
    };
    // SAFETY: buffer corresponds fsverity_digest And it holds SHA-256.
    assert_eq!(
        unsafe { libc::ioctl(file.as_raw_fd(), MEASURE, &mut digest) },
        0
    );
    assert_eq!(digest.algorithm, SHA256);
    assert_eq!(digest.size, 32);
    assert_eq!(measured, digest.bytes);
    assert!(matches_approved_digest(file.as_fd(), &digest.bytes).unwrap());
    let mut wrong = digest.bytes;
    wrong[0] ^= 1;
    assert!(!matches_approved_digest(file.as_fd(), &wrong).unwrap());
    assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
}

#[test]
fn ordinary_file_is_not_accepted_as_verified() {
    let path = std::env::temp_dir().join(format!(
        "carosign-verity-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    assert!(matches_approved_digest(file.as_fd(), &[0; 32]).is_err());
    std::fs::remove_file(path).unwrap();
}
