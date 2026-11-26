#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MatchedEvidenceStage {
    Ready,
    Evaluating,
    Evaluated,
    Applied,
    HandedOff,
}
