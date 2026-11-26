use super::{common, named_enrollment};

#[test]
fn named_verification_requires_an_unambiguous_ready_enrollment() {
    let reference = common::EnrollmentRef {
        enrollment_id: vec![1; 16],
        ..Default::default()
    };
    let mut entry = common::EnrollmentSummary {
        enrollment: Some(reference.clone()),
        label: "left-index-finger".into(),
        ..Default::default()
    };
    let inventory = std::slice::from_ref(&entry);
    assert_eq!(
        named_enrollment(inventory, "left-index-finger"),
        Some(reference)
    );
    assert_eq!(named_enrollment(inventory, "right-index-finger"), None);
    assert_eq!(
        named_enrollment(&[entry.clone(), entry.clone()], "left-index-finger"),
        None
    );
    let mut other = entry.clone();
    other.label = "right-thumb".into();
    assert_eq!(
        named_enrollment(&[entry.clone(), other], "left-index-finger"),
        entry.enrollment.clone(),
    );
    entry.pending_reconciliation = true;
    assert_eq!(named_enrollment(&[entry], "left-index-finger"), None);
}
