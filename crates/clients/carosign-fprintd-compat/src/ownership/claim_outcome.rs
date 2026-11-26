use super::Owner;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClaimOutcome {
    Granted,
    Refused {
        by: Owner,
    },
}
