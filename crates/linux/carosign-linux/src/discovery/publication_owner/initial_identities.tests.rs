use super::*;

#[test]
fn initial_identities_reject_zero_repeat_and_random_failure() {
    let mut values = [[1; 16], [2; 16], [3; 16]].into_iter();
    let ids = allocate_initial_identities(|| values.next()).unwrap();
    assert_eq!(ids.package.as_bytes(), &[1; 16]);
    assert_eq!(ids.provider.as_bytes(), &[2; 16]);
    assert_eq!(ids.profile.as_bytes(), &[3; 16]);

    let mut repeated = [[1; 16], [2; 16], [1; 16]].into_iter();
    assert!(matches!(
        allocate_initial_identities(|| repeated.next()),
        Err(PreparationError::IdUnavailable)
    ));
    let mut zero = [[0; 16], [2; 16], [3; 16]].into_iter();
    assert!(matches!(
        allocate_initial_identities(|| zero.next()),
        Err(PreparationError::IdUnavailable)
    ));
    assert!(matches!(
        allocate_initial_identities(|| None),
        Err(PreparationError::IdUnavailable)
    ));
}
