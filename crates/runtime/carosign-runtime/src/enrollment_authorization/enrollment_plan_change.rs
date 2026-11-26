#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnrollmentPlanChange {
    Create,
    Replace {
        old: carosign_core::enrollment::EnrollmentRef,
    },
}
