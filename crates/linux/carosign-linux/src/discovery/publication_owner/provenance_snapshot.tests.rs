use super::super::{duplicate_fd::duplicate_fd, stage_provenance::stage_provenance};
use super::*;
use std::os::unix::fs::FileExt;

#[test]
fn provenance_survives_reopen_and_rejects_replacement_and_name_collision() {
    use std::os::unix::fs::OpenOptionsExt;
    let dir = std::env::temp_dir().join(format!(
        "carosign-origin-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir(&dir).unwrap();
    let root = std::fs::File::open(&dir).unwrap();
    let snapshot_path = dir.join("input");
    std::fs::write(&snapshot_path, b"signed archive").unwrap();
    let snapshot = std::fs::File::open(&snapshot_path).unwrap();
    let saved = stage_provenance(
        root.as_fd(),
        PROVENANCE_ARTIFACT,
        snapshot.as_fd(),
        1024,
        |_| Ok([1; 32]),
    )
    .unwrap();
    drop(saved);
    assert_eq!(
        std::fs::read(dir.join(PROVENANCE_ARTIFACT.to_str().unwrap())).unwrap(),
        b"signed archive"
    );
    assert_eq!(
        stage_provenance(
            root.as_fd(),
            PROVENANCE_ARTIFACT,
            snapshot.as_fd(),
            1024,
            |_| Ok([1; 32])
        )
        .unwrap_err()
        .kind(),
        io::ErrorKind::AlreadyExists
    );
    let path = dir.join(PROVENANCE_SIGNATURE.to_str().unwrap());
    let result = stage_provenance(
        root.as_fd(),
        PROVENANCE_SIGNATURE,
        snapshot.as_fd(),
        1024,
        |_| {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&path)?;
            file.write_all_at(b"X", 0)?;
            Ok([1; 32])
        },
    );
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    drop(root);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reopened_provenance_rejects_missing_changed_or_symlink() {
    use std::os::unix::fs::symlink;
    let path = std::env::temp_dir().join(format!(
        "carosign-reopen-provenance-{}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    let root = std::fs::File::open(&path).unwrap();
    let artifact = path.join(".carosign-origin-artifact");
    let signature = path.join(".carosign-origin-signature");
    std::fs::write(&artifact, b"archive").unwrap();
    std::fs::write(&signature, b"signature").unwrap();
    let uid = unsafe { libc::geteuid() };
    let expected = [[1; 32], [2; 32]];
    let check = |fd: BorrowedFd<'_>, digest: &[u8; 32]| {
        let file = std::fs::File::from(duplicate_fd(fd)?);
        let mut data = [0; 16];
        let size = file.read_at(&mut data, 0)?;
        Ok((digest == &[1; 32] && &data[..size] == b"archive")
            || (digest == &[2; 32] && &data[..size] == b"signature"))
    };
    let reopened = reopen_provenance_with(root.as_fd(), uid, &expected, check).unwrap();
    assert_ne!(
        reopened.artifact.as_raw_fd(),
        reopened.signature.as_raw_fd()
    );
    drop(reopened);
    std::fs::write(&signature, b"tampered!").unwrap();
    assert!(reopen_provenance_with(root.as_fd(), uid, &expected, check).is_err());
    std::fs::remove_file(&signature).unwrap();
    assert!(reopen_provenance_with(root.as_fd(), uid, &expected, check).is_err());
    symlink(&artifact, &signature).unwrap();
    assert!(reopen_provenance_with(root.as_fd(), uid, &expected, check).is_err());
    drop(root);
    std::fs::remove_dir_all(path).unwrap();
}
