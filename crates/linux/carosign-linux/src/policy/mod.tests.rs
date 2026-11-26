use std::os::fd::AsRawFd;
use std::path::Path;

use super::*;
use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

// All observation tests use a separate temporary FS and one interprocess lock: creating
// adjacent catalogs changes the attributes of the ancestor.
fn isolated() -> std::fs::File {
    let lock = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open("/dev/shm/carosign-policy-tests.lock")
        .unwrap();
    assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) }, 0);
    lock
}

fn fixture() -> std::path::PathBuf {
    let dir = std::path::Path::new("/dev/shm").join(format!(
        "carosign-policy-watch-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("policy"), b"original").unwrap();
    dir
}

#[test]
fn write_invalidates_and_cannot_be_cleared() {
    let _lock = isolated();
    let dir = fixture();
    let path = dir.join("policy");
    let mut watch = PolicyChangeWatch::new(&path).unwrap();
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Unchanged);
    std::fs::write(&path, b"changed").unwrap();
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Invalidated);
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Invalidated);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unrelated_file_in_policy_directory_does_not_revoke_policy() {
    let _lock = isolated();
    let dir = fixture();
    let path = dir.join("policy");
    let mut watch = PolicyChangeWatch::new(&path).unwrap();
    std::fs::write(dir.join("other-profile"), b"other").unwrap();
    std::fs::remove_file(dir.join("other-profile")).unwrap();
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Unchanged);
    std::fs::write(&path, b"updated").unwrap();
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Invalidated);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unrelated_ancestor_entry_attributes_do_not_revoke_policy() {
    use std::os::unix::fs::PermissionsExt;
    let _lock = isolated();
    let dir = fixture();
    let nested = dir.join("nested");
    std::fs::create_dir(&nested).unwrap();
    let path = nested.join("policy");
    std::fs::write(&path, b"policy").unwrap();
    let sibling = dir.join("unrelated");
    std::fs::write(&sibling, b"unrelated").unwrap();
    let mut watch = PolicyChangeWatch::new(&path).unwrap();
    std::fs::set_permissions(&sibling, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Unchanged);
    std::fs::rename(&nested, dir.join("moved")).unwrap();
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Invalidated);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn other_temporary_tree_does_not_invalidate_isolated_watch() {
    let _lock = isolated();
    let dir = fixture();
    let mut watch = PolicyChangeWatch::new(&dir.join("policy")).unwrap();
    let other = std::env::temp_dir().join(format!(
        "carosign-policy-neighbor-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&other).unwrap();
    std::fs::remove_dir(&other).unwrap();
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Unchanged);
    std::fs::write(dir.join("policy"), b"changed").unwrap();
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Invalidated);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn atomic_replacement_invalidates_old_inode() {
    let _lock = isolated();
    let dir = fixture();
    let path = dir.join("policy");
    let mut watch = PolicyChangeWatch::new(&path).unwrap();
    std::fs::write(dir.join("replacement"), b"new").unwrap();
    std::fs::rename(dir.join("replacement"), path).unwrap();
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Invalidated);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn parent_directory_move_invalidates_watch() {
    let _lock = isolated();
    let dir = fixture();
    let mut watch = PolicyChangeWatch::new(&dir.join("policy")).unwrap();
    let moved = dir.with_extension("moved");
    std::fs::rename(&dir, &moved).unwrap();
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Invalidated);
    std::fs::remove_dir_all(moved).unwrap();
}

#[test]
fn ancestor_directory_move_invalidates_watch() {
    let _lock = isolated();
    let dir = fixture();
    let nested = dir.join("nested");
    std::fs::create_dir(&nested).unwrap();
    let path = nested.join("policy");
    std::fs::write(&path, b"policy").unwrap();
    let mut watch = PolicyChangeWatch::new(&path).unwrap();
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Unchanged);
    let moved = dir.with_extension("moved");
    std::fs::rename(&dir, &moved).unwrap();
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Invalidated);
    std::fs::remove_dir_all(moved).unwrap();
}

#[test]
fn symlinked_ancestor_is_rejected() {
    let _lock = isolated();
    let dir = fixture();
    let nested = dir.join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("policy"), b"policy").unwrap();
    symlink(&nested, dir.join("alias")).unwrap();
    assert!(PolicyChangeWatch::new(&dir.join("alias/policy")).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn missing_and_symlink_are_rejected() {
    let _lock = isolated();
    let dir = fixture();
    assert!(PolicyChangeWatch::new(&dir.join("missing")).is_err());
    symlink(dir.join("policy"), dir.join("link")).unwrap();
    assert!(PolicyChangeWatch::new(&dir.join("link")).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn protected_snapshot_rejects_writable_ancestor_and_symlink() {
    let _lock = isolated();
    let dir = fixture();
    assert!(ProtectedPolicySnapshot::open(&dir.join("policy")).is_err());
    symlink(dir.join("policy"), dir.join("link")).unwrap();
    assert!(ProtectedPolicySnapshot::open(&dir.join("link")).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn protected_snapshot_reads_real_root_owned_file() {
    let snapshot = ProtectedPolicySnapshot::open(Path::new("/etc/passwd")).unwrap();
    let (contents, mut watch) = snapshot.into_parts();
    assert!(!contents.is_empty());
    assert_eq!(watch.check().unwrap(), PolicyWatchState::Unchanged);
}
