use super::super::*;

#[test]
fn only_one_owner() {
    assert_eq!(
        claim(Owner::Platform, Owner::SystemFprintd),
        ClaimOutcome::Refused {
            by: Owner::Platform
        }
    );
    assert_eq!(claim(Owner::None, Owner::Platform), ClaimOutcome::Granted);
}

#[test]
fn foreign_release_does_nothing() {
    assert_eq!(
        release(Owner::Platform, Owner::SystemFprintd),
        Owner::Platform
    );
    assert_eq!(release(Owner::Platform, Owner::Platform), Owner::None);
}
