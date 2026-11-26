use carosign_protocol::carosign::{
    client::v1::Authenticators,
    common::v1::{AuthenticatorDescriptor, CapabilityDescriptor, EnrollmentRef, EnrollmentSummary},
};

use super::*;

#[test]
fn live_commands_target_the_current_scanner_and_exact_enrollment_generation() {
    let fingerprints = Fingerprints::new().unwrap();
    let localizer = Localizer::from_environment();
    let reference = EnrollmentRef {
        authenticator_id: vec![1; 16],
        enrollment_id: vec![2; 16],
        generation: 3,
    };
    let snapshot = Snapshot {
        health: Err(QueryError::Unavailable),
        inventory: Ok(Authenticators {
            entries: vec![AuthenticatorDescriptor {
                available: true,
                authenticator_id: reference.authenticator_id.clone(),
                capabilities: Some(CapabilityDescriptor {
                    modality: Modality::Fingerprint as i32,
                    operations: vec![ProviderOperation::ProviderEnroll as i32],
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        enrollments: Ok(vec![EnrollmentSummary {
            enrollment: Some(reference.clone()),
            label: "Right index finger".into(),
            original_label: None,
            pending_reconciliation: false,
        }]),
    };

    fingerprints.render(Some(&snapshot), &localizer);
    for discard in [false, true] {
        let (true, Some(FingerprintJob::Enroll { device })) =
            fingerprints.live_command("fingerprint_add")
        else {
            panic!("scan did not start")
        };
        assert_eq!(device, reference.authenticator_id);
        assert!(!fingerprints.edit_enrollment("Too early"));
        assert!(matches!(
            fingerprints.live_command("fingerprint_next"),
            (false, None)
        ));
        let captured = EnrollmentRef {
            enrollment_id: vec![if discard { 8 } else { 7 }; 16],
            ..reference.clone()
        };
        fingerprints.apply_live_update(FingerprintUpdate::Captured(Ok(captured.clone())));
        assert!(fingerprints.is_enrolling());
        if discard {
            let (true, Some(FingerprintJob::Remove(target))) =
                fingerprints.live_command("fingerprint_discard")
            else {
                panic!("discard did not start")
            };
            assert_eq!(target, captured);
            fingerprints.apply_live_update(FingerprintUpdate::Finished(Err(
                "ui-fingerprint-delete-failed",
            )));
            assert!(fingerprints.is_enrolling());
            assert!(matches!(
                fingerprints.live_command("fingerprint_discard"),
                (true, Some(FingerprintJob::Remove(_)))
            ));
            fingerprints.apply_live_update(FingerprintUpdate::Finished(Ok(())));
        } else {
            assert!(matches!(
                fingerprints.live_command("fingerprint_next"),
                (false, None)
            ));
            assert!(fingerprints.edit_enrollment(" My thumb "));
            let (
                true,
                Some(FingerprintJob::Rename {
                    reference: target,
                    expected,
                    label,
                }),
            ) = fingerprints.live_command("fingerprint_next")
            else {
                panic!("name save did not start")
            };
            assert_eq!(target, captured);
            assert!(expected.is_empty());
            assert_eq!(label, "My thumb");
            fingerprints.apply_live_update(FingerprintUpdate::Renamed {
                reference: target,
                expected,
                label,
                result: Ok(()),
            });
        }
        assert!(!fingerprints.is_enrolling());
    }

    let row_id = LiveFingerprints::row_id(&reference);
    assert!(fingerprints.select_registered(&row_id));
    assert!(matches!(
        fingerprints.live_command("fingerprint_rename"),
        (true, None)
    ));
    assert!(fingerprints.edit_registered_name(" New name "));
    let (
        true,
        Some(FingerprintJob::Rename {
            reference: target,
            expected,
            label,
        }),
    ) = fingerprints.live_command("fingerprint_rename_save")
    else {
        panic!("rename did not start")
    };
    assert_eq!(target, reference);
    assert_eq!(expected, "Right index finger");
    assert_eq!(label, "New name");
    fingerprints.apply_live_update(FingerprintUpdate::Renamed {
        reference: target,
        expected,
        label,
        result: Err("ui-fingerprint-rename-conflict"),
    });
    let mut refreshed = snapshot.clone();
    refreshed.enrollments.as_mut().unwrap()[0].label = "Newer server name".into();
    fingerprints.render(Some(&refreshed), &localizer);
    let (
        true,
        Some(FingerprintJob::Rename {
            reference: target,
            expected,
            label,
        }),
    ) = fingerprints.live_command("fingerprint_rename_save")
    else {
        panic!("rename draft did not survive conflict")
    };
    assert_eq!(expected, "Newer server name");
    assert_eq!(label, "New name");
    fingerprints.apply_live_update(FingerprintUpdate::Renamed {
        reference: target,
        expected,
        label,
        result: Ok(()),
    });
    fingerprints.render(Some(&refreshed), &localizer);
    assert_eq!(fingerprints.live.borrow().rows[0].label, "New name");
    assert!(matches!(
        fingerprints.live_command("fingerprint_delete"),
        (true, None)
    ));
    assert!(matches!(
        fingerprints.live_command("fingerprint_delete_cancel"),
        (true, None)
    ));
    assert!(matches!(
        fingerprints.live_command("fingerprint_delete_confirm"),
        (false, None)
    ));
    assert!(matches!(
        fingerprints.live_command("fingerprint_delete"),
        (true, None)
    ));
    let (true, Some(FingerprintJob::Remove(target))) =
        fingerprints.live_command("fingerprint_delete_confirm")
    else {
        panic!("removal did not start")
    };
    assert_eq!(target, reference);
    fingerprints.apply_live_update(FingerprintUpdate::Finished(Ok(())));
    fingerprints.render(Some(&snapshot), &localizer);
    assert!(!fingerprints.select_registered(&row_id));
}
