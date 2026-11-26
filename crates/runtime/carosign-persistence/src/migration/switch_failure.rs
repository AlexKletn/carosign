/// Ambiguous fixation requires reading a new picture, not repeating it.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum SwitchFailure {
    Invalid,
    CommitUnknown,
}
