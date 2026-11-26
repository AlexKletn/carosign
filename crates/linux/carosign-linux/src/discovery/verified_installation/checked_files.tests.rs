use crate::discovery::verified_installation::{ApprovedFile, KernelVerity};
use crate::discovery::verified_installation::{
    changed_authority::ChangedAuthority, must_not_measure::MustNotMeasure,
};
use carosign_core::ids::ProfileId;
use std::cell::Cell;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use super::{FileRole, InstallationKey, Refusal, UnavailableAuthority, validate};
use crate::discovery::verified_installation::{
    fixture_measure::FixtureMeasure, installation_fixture::Fixture, runtime_decoder::RuntimeDecoder,
};
use crate::sys;

#[test]
fn installation_record_bounds_are_checked_before_measuring_files() {
    let mut fixture = Fixture::new();
    let original = fixture.record.clone();
    for invalid in [
        "bad//path".to_owned(),
        "bad/../path".to_owned(),
        "bad/\npath".to_owned(),
        format!("{}{}", "a/".repeat(512), "b"),
    ] {
        fixture.record.files.insert(
            invalid,
            ApprovedFile {
                role: FileRole::Runtime,
                digest: [7; 32],
            },
        );
        assert!(matches!(
            fixture.run(fixture.record.key, &MustNotMeasure),
            Err(Refusal::InvalidData)
        ));
        fixture.record = original.clone();
    }
    for n in 0..124 {
        fixture.record.files.insert(
            format!("extra/{n}"),
            ApprovedFile {
                role: FileRole::Runtime,
                digest: [7; 32],
            },
        );
    }
    assert_eq!(fixture.record.files.len(), 129);
    assert!(matches!(
        fixture.run(fixture.record.key, &MustNotMeasure),
        Err(Refusal::InvalidData)
    ));
}

#[test]
fn exact_identity_policy_and_every_runtime_measurement_are_required() {
    let mut fixture = Fixture::new();
    let measure = FixtureMeasure { fail_inode: None };
    let checked = fixture.run(fixture.record.key, &measure).unwrap();
    assert_eq!(checked.policy.package, "org.example.provider");
    assert_eq!(checked.runtime.len(), 1);
    let wrong = InstallationKey {
        profile: ProfileId::from_bytes([9; 16]).unwrap(),
        ..fixture.record.key
    };
    assert!(matches!(
        fixture.run(wrong, &measure),
        Err(Refusal::Mismatch)
    ));
    for path in [
        "policy",
        "package.toml",
        "profiles/default.toml",
        "worker",
        "lib/runtime",
    ] {
        let ino = std::fs::metadata(fixture.root.join(path)).unwrap().ino();
        assert!(
            matches!(
                fixture.run(
                    fixture.record.key,
                    &FixtureMeasure {
                        fail_inode: Some(ino)
                    }
                ),
                Err(Refusal::Mismatch)
            ),
            "{path}"
        );
    }
    fixture.revoked.set(true);
    assert!(matches!(
        fixture.run(fixture.record.key, &measure),
        Err(Refusal::Unavailable)
    ));
    fixture.revoked.set(false);
    fixture.unavailable.set(true);
    assert!(matches!(
        fixture.run(fixture.record.key, &measure),
        Err(Refusal::Unavailable)
    ));
    fixture.unavailable.set(false);
    fixture.record.version = "2.0.0".into();
    assert!(matches!(
        fixture.run(fixture.record.key, &measure),
        Err(Refusal::Mismatch)
    ));
}

#[test]
fn retained_files_are_revalidated_before_launch_handoff() {
    let mut fixture = Fixture::new();
    let measure = FixtureMeasure { fail_inode: None };
    let checked = fixture.run(fixture.record.key, &measure).unwrap();
    checked.revalidate(&fixture, &measure).unwrap();

    fixture.revoked.set(true);
    assert_eq!(
        checked.revalidate(&fixture, &measure),
        Err(Refusal::Unavailable)
    );
    fixture.revoked.set(false);

    fixture.record.revision += 1;
    assert_eq!(
        checked.revalidate(&fixture, &measure),
        Err(Refusal::Changed)
    );
    fixture.record.revision -= 1;

    let ino = std::fs::metadata(fixture.root.join("lib/runtime"))
        .unwrap()
        .ino();
    assert_eq!(
        checked.revalidate(
            &fixture,
            &FixtureMeasure {
                fail_inode: Some(ino)
            }
        ),
        Err(Refusal::Mismatch)
    );
    std::fs::set_permissions(
        fixture.root.join("worker"),
        std::fs::Permissions::from_mode(0o666),
    )
    .unwrap();
    assert_eq!(
        checked.revalidate(&fixture, &measure),
        Err(Refusal::Mismatch)
    );
}

#[test]
fn retained_file_revalidation_rejects_changed_parent_and_replaced_path() {
    let fixture = Fixture::new();
    let measure = FixtureMeasure { fail_inode: None };
    let checked = fixture.run(fixture.record.key, &measure).unwrap();

    // Even a fixed descriptor does not confirm the previous rights of the catalog.
    let dir = fixture.root.join("lib");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
    assert!(matches!(
        checked.revalidate(&fixture, &measure),
        Err(Refusal::UnsafeObject)
    ));
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    checked.revalidate(&fixture, &measure).unwrap();

    // The matching role and measurement do not make the new inode the same file.
    let old = fixture.root.join("lib/old-runtime");
    let runtime = fixture.root.join("lib/runtime");
    std::fs::rename(&runtime, &old).unwrap();
    std::fs::write(&runtime, b"runtime").unwrap();
    assert!(matches!(
        checked.revalidate(&fixture, &measure),
        Err(Refusal::Changed)
    ));
}

#[test]
fn approved_digest_cannot_change_a_files_role() {
    let mut fixture = Fixture::new();
    let measure = FixtureMeasure { fail_inode: None };
    for (path, wrong) in [
        ("policy", FileRole::Manifest),
        ("package.toml", FileRole::Policy),
        ("profiles/default.toml", FileRole::Runtime),
        ("worker", FileRole::Runtime),
        ("lib/runtime", FileRole::Executable),
    ] {
        let original = fixture.record.files.get(path).unwrap().role;
        fixture.record.files.get_mut(path).unwrap().role = wrong;
        assert!(
            matches!(
                fixture.run(fixture.record.key, &measure),
                Err(Refusal::Mismatch)
            ),
            "{path}"
        );
        fixture.record.files.get_mut(path).unwrap().role = original;
    }
    assert!(fixture.run(fixture.record.key, &measure).is_ok());
}

#[test]
fn changed_approved_record_is_rejected_before_handoff() {
    let fixture = Fixture::new();
    for change in 0..8 {
        let source = ChangedAuthority {
            fixture: &fixture,
            reads: Cell::new(0),
            change,
        };
        let root = sys::open_launch_candidate(fixture.root.to_str().unwrap()).unwrap();
        assert!(
            matches!(
                validate(
                    &source,
                    &FixtureMeasure { fail_inode: None },
                    &RuntimeDecoder,
                    fixture.record.key,
                    root,
                    fixture.owner,
                ),
                Err(Refusal::Changed)
            ),
            "изменение {change} принято"
        );
        assert_eq!(source.reads.get(), 2);
    }
}

#[test]
fn missing_runtime_and_unavailable_system_source_refuse() {
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.root.join("lib/runtime")).unwrap();
    assert!(matches!(
        fixture.run(fixture.record.key, &FixtureMeasure { fail_inode: None }),
        Err(Refusal::Kernel(libc::ENOENT))
    ));
    let root = sys::open_launch_candidate(fixture.root.to_str().unwrap()).unwrap();
    assert!(matches!(
        validate(
            &UnavailableAuthority,
            &KernelVerity,
            &RuntimeDecoder,
            fixture.record.key,
            root,
            fixture.owner
        ),
        Err(Refusal::Unavailable)
    ));
}

#[test]
fn parsed_declarations_must_match_the_approved_scope() {
    let fixture = Fixture::new();
    let measure = FixtureMeasure { fail_inode: None };
    for (file, original, changed) in [
        ("policy", "org.example.provider", "org.example.other"),
        ("policy", "profile='default'", "profile='other'"),
        ("package.toml", "version='1.0.0'", "version='2.0.0'"),
        ("package.toml", "executable='worker'", "executable='other'"),
        ("profiles/default.toml", "id='default'", "id='other'"),
    ] {
        let path = fixture.root.join(file);
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains(original));
        std::fs::write(&path, saved.replace(original, changed)).unwrap();
        assert!(
            matches!(
                fixture.run(fixture.record.key, &measure),
                Err(Refusal::Mismatch)
            ),
            "{file}: {original}"
        );
        std::fs::write(&path, saved).unwrap();
    }
}
