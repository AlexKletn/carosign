use super::super::supervisor_policy::SupervisorPolicy;
use super::{fixture_decoder::Decoder, stage_directory::StageDirectory};
use std::io::Read;
use std::os::fd::OwnedFd;

use super::*;
use std::os::fd::AsFd;
use std::os::unix::fs::{PermissionsExt, symlink};
fn append(builder: &mut tar::Builder<Vec<u8>>, name: &str, kind: tar::EntryType, body: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(kind);
    header.set_size(body.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, name, body).unwrap();
}
fn fixture(extra: Option<(&str, tar::EntryType)>) -> Vec<u8> {
    let mut b = tar::Builder::new(Vec::new());
    append(&mut b, "package.toml", tar::EntryType::Regular, b"manifest");
    append(&mut b, "bin/worker", tar::EntryType::Regular, b"elf");
    append(
        &mut b,
        "profiles/default.toml",
        tar::EntryType::Regular,
        b"profile",
    );
    if let Some((name, kind)) = extra {
        append(&mut b, name, kind, b"");
    }
    b.into_inner().unwrap()
}
#[test]
fn accepts_regular_files_and_rejects_duplicates_links_and_parent_file() {
    let valid = inspect_bytes(&fixture(None), &Decoder, "default").unwrap();
    assert_eq!(valid.files["bin/worker"], b"elf");
    for (name, kind) in [
        ("bin/worker", tar::EntryType::Regular),
        ("bad", tar::EntryType::Symlink),
        ("hardlink", tar::EntryType::Link),
        ("device", tar::EntryType::Char),
        ("extension", tar::EntryType::XHeader),
        ("bin", tar::EntryType::Regular),
        ("bin/worker/subdir", tar::EntryType::Directory),
    ] {
        assert!(inspect_bytes(&fixture(Some((name, kind))), &Decoder, "default").is_err());
    }
    assert!(safe_path(b"../outside", false).is_err());
    assert!(safe_path(b"/absolute", false).is_err());
    let mut concatenated = fixture(None);
    concatenated.extend_from_slice(&fixture(None));
    assert!(inspect_bytes(&concatenated, &Decoder, "default").is_err());
}

#[test]
fn stages_only_into_private_new_tree_without_following_links() {
    let temp = std::env::temp_dir().join(format!("carosign-stage-{}", std::process::id()));
    std::fs::create_dir(&temp).unwrap();
    let _cleanup = StageDirectory(temp.clone());
    std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o700)).unwrap();
    let root = std::fs::File::open(&temp).unwrap();
    let checked = inspect_bytes(&fixture(None), &Decoder, "default").unwrap();
    let staged = stage(&checked, root.as_fd()).unwrap();
    assert_eq!(staged.len(), 3);
    let mut bytes = [0; 3];
    std::fs::File::from(staged["bin/worker"].try_clone().unwrap())
        .read_exact(&mut bytes)
        .unwrap();
    assert_eq!(&bytes, b"elf");
    assert!(stage(&checked, root.as_fd()).is_err());

    let other = temp.join("other");
    std::fs::create_dir(&other).unwrap();
    std::fs::set_permissions(&other, std::fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&temp, other.join("bin")).unwrap();
    let other_root = std::fs::File::open(&other).unwrap();
    assert!(stage(&checked, other_root.as_fd()).is_err());
    std::fs::set_permissions(&other, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        stage(&checked, other_root.as_fd()),
        Err(ArchiveError::Unsafe)
    ));
}

#[test]
fn reopened_staged_file_rejects_replacement_with_same_length() {
    let temp =
        std::env::temp_dir().join(format!("carosign-stage-identity-{}", std::process::id()));
    std::fs::create_dir(&temp).unwrap();
    let path = temp.join("file");
    std::fs::write(&path, b"original").unwrap();
    let written = std::fs::File::open(&path).unwrap();
    let before = written.metadata().unwrap();
    assert!(same_staged_file(
        &before,
        &std::fs::metadata(&path).unwrap()
    ));
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"replaced").unwrap();
    assert!(!same_staged_file(
        &before,
        &std::fs::metadata(&path).unwrap()
    ));
    drop(written);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn signed_bytes_reject_same_inode_same_length_change() {
    let temp = std::env::temp_dir().join(format!(
        "carosign-stage-signed-content-{}",
        std::process::id()
    ));
    std::fs::create_dir(&temp).unwrap();
    let path = temp.join("file");
    std::fs::write(&path, b"original").unwrap();
    let file: OwnedFd = std::fs::File::open(&path).unwrap().into();
    let before = std::fs::metadata(&path).unwrap();
    assert!(matches_signed_bytes(&file, b"original").unwrap());
    std::fs::write(&path, b"modified").unwrap();
    let after = std::fs::metadata(&path).unwrap();
    assert!(same_staged_file(&before, &after));
    assert!(!matches_signed_bytes(&file, b"original").unwrap());
    assert!(!matches_signed_bytes(&file, b"original-extra").unwrap());
    drop(file);
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn policy_binds_signed_manifest_and_exact_runtime_set() {
    let valid = format!(
        "schema=1\npackage='org.example.test'\nversion='1'\nprofile='default'\narchitecture='{}'\nexecutable='bin/worker'\nuid=42\ngid=43\nmemory_bytes=4096\ncpu_quota_us=5000\ncpu_period_us=10000\nprocesses=1\nruntime=[]\nsyscalls=[0,1]\n",
        std::env::consts::ARCH
    );
    let checked = inspect_bytes(&fixture(None), &Decoder, "default").unwrap();
    let policy = SupervisorPolicy::parse(valid.as_bytes()).unwrap();
    check_policy(&checked, &Decoder, &policy).unwrap();
    for altered in [
        valid.replace("org.example.test", "org.example.other"),
        valid.replace("version='1'", "version='2'"),
        valid.replace("bin/worker", "bin/other"),
        valid.replace("runtime=[]", "runtime=['missing']"),
        valid.replace("runtime=[]", "runtime=['package.toml']"),
    ] {
        let policy = SupervisorPolicy::parse(altered.as_bytes()).unwrap();
        assert!(check_policy(&checked, &Decoder, &policy).is_err());
    }
    let extra = inspect_bytes(
        &fixture(Some(("extra", tar::EntryType::Regular))),
        &Decoder,
        "default",
    )
    .unwrap();
    assert!(check_policy(&extra, &Decoder, &policy).is_err());
}
