use super::{RecoveryCheckedFiles, RecoveryFileCandidate, check_recovery_files_with};
use crate::discovery::verified_installation::{
    fixture_measure::FixtureMeasure, installation_fixture::Fixture,
};
use crate::sys;

use std::collections::BTreeMap;
use std::os::fd::OwnedFd;

#[test]
fn recovery_content_compares_exact_bytes_and_file_set() {
    let fixture = Fixture::new();
    let root: OwnedFd = std::fs::File::open(&fixture.root).unwrap().into();
    let path = fixture.root.join("recovery-content");
    std::fs::write(&path, b"signed").unwrap();
    let fd: OwnedFd = std::fs::File::open(&path).unwrap().into();
    let files = RecoveryCheckedFiles {
        root_identity: sys::launch_metadata(&root).unwrap().identity,
        root,
        files: vec![(
            RecoveryFileCandidate {
                path: "recovery-content".into(),
                digest: [1; 32],
            },
            fd,
        )],
    };
    let mut expected = BTreeMap::from([("recovery-content".into(), b"signed".to_vec())]);
    assert!(files.matches_contents(&expected));
    expected.insert("recovery-content".into(), b"SIGNed".to_vec());
    assert!(!files.matches_contents(&expected));
    expected.insert("recovery-content".into(), b"signed".to_vec());
    expected.insert("extra".into(), vec![]);
    assert!(!files.matches_contents(&expected));
    expected.remove("extra");
    std::fs::write(&path, b"tamper").unwrap();
    assert!(!files.matches_contents(&expected));
}

#[test]
fn recovery_files_detect_path_replacement_and_bad_candidates() {
    let fixture = Fixture::new();
    let measure = FixtureMeasure { fail_inode: None };
    let root = || sys::open_launch_candidate(fixture.root.to_str().unwrap()).unwrap();
    let candidate = || RecoveryFileCandidate {
        path: "worker".into(),
        digest: [7; 32],
    };
    assert!(
        check_recovery_files_with(
            root(),
            vec![candidate(), candidate()],
            fixture.owner,
            &measure
        )
        .is_err()
    );
    assert!(
        check_recovery_files_with(
            root(),
            vec![RecoveryFileCandidate {
                path: "../worker".into(),
                digest: [7; 32]
            }],
            fixture.owner,
            &measure
        )
        .is_err()
    );
    let checked =
        check_recovery_files_with(root(), vec![candidate()], fixture.owner, &measure).unwrap();
    checked.revalidate_with(fixture.owner, &measure).unwrap();
    std::fs::rename(fixture.root.join("worker"), fixture.root.join("old-worker")).unwrap();
    std::fs::write(fixture.root.join("worker"), b"replacement").unwrap();
    assert!(checked.revalidate_with(fixture.owner, &measure).is_err());
}
