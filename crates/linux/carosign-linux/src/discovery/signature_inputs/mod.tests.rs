use std::io::Write;
use std::os::fd::AsFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;

use super::*;
use std::io::{Read, Seek, SeekFrom};

#[test]
fn primary_key_version_does_not_guess_32_byte_fingerprint() {
    let v4 = "BCC1653DA4E4393ADE89C8208E6D399CC45DFE81";
    let v5 = "9D02E1F843B3582E5EE7C39D88FAA866A4A95E9657A8D9B2F217CFDEDFF0728D";
    let v6 = "8E60940C90761283F524F0A891579F6B06FF62533FB20279426CD65F469361C6";
    let combined = snapshot_file(&[
        0xc6, 6, 4, 0, 0, 0, 0, 1, 0xc6, 6, 5, 0, 0, 0, 0, 1, 0xc6, 6, 6, 0, 0, 0, 0, 1,
    ]);
    for (fingerprint, version) in [(v4, 4), (v5, 5), (v6, 6)] {
        assert_eq!(
            primary_version(combined.as_fd(), fingerprint).unwrap(),
            version
        );
    }
    assert!(primary_version(combined.as_fd(), &"A".repeat(64)).is_err());
    let duplicate = snapshot_file(&[0xc6, 6, 4, 0, 0, 0, 0, 1, 0xc6, 6, 4, 0, 0, 0, 0, 1]);
    assert!(primary_version(duplicate.as_fd(), v4).is_err());
    let partial = snapshot_file(&[0xc6, 224, 4]);
    assert!(primary_version(partial.as_fd(), v4).is_err());
}

#[test]
fn snapshot_is_sealed_and_independent_of_source() {
    let mut source = std::fs::File::from(snapshot_file(b"before"));
    let pinned = snapshot(source.as_fd(), 64).unwrap();
    source.seek(SeekFrom::Start(0)).unwrap();
    source.write_all(b"after!").unwrap();
    let mut copy = std::fs::File::from(pinned);
    copy.seek(SeekFrom::Start(0)).unwrap();
    let mut data = Vec::new();
    copy.read_to_end(&mut data).unwrap();
    assert_eq!(data, b"before");
    assert!(copy.write_all(b"x").is_err());
}

fn snapshot_file(data: &[u8]) -> OwnedFd {
    // SAFETY: Create a separate anonymous test file.
    let fd = unsafe { libc::memfd_create(c"fixture".as_ptr(), libc::MFD_CLOEXEC) };
    assert!(fd >= 0);
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    file.write_all(data).unwrap();
    file.into()
}

#[test]
fn limits_and_nonregular_inputs_fail() {
    let file = snapshot_file(b"12345");
    assert!(matches!(
        snapshot(file.as_fd(), 4),
        Err(InputError::TooLarge)
    ));
    let directory = std::fs::File::open("/").unwrap();
    assert!(matches!(
        snapshot(directory.as_fd(), 100),
        Err(InputError::InvalidFile)
    ));
}

#[test]
fn ordinary_keyring_does_not_become_trusted() {
    let file = snapshot_file(b"not a verity keyring");
    assert!(SignatureInputs::prepare(file.as_fd(), file.as_fd(), file.as_fd(), &[0; 32]).is_err());
}

#[test]
fn unmeasured_verifier_and_nonregular_verifier_are_rejected() {
    let file = snapshot_file(b"not a measured executable");
    assert!(PinnedVerifier::prepare(file.as_fd(), &[0; 32]).is_err());
    let directory = std::fs::File::open("/").unwrap();
    assert!(matches!(
        PinnedVerifier::prepare(directory.as_fd(), &[0; 32]),
        Err(InputError::InvalidFile)
    ));
}
