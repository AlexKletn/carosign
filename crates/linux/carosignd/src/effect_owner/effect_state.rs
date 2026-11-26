#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EffectState {
    Pending,
    Claimed,
    Uncertain,
}
