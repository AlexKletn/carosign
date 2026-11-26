use super::Step;

pub(super) enum StartStage {
    Bound(Step),
    HandedOff,
    Finished,
}
