use super::evidence_evaluation::EvidenceEvaluation;

pub(super) enum EvidenceEvaluationSlot {
    Running,
    Ready(Box<EvidenceEvaluation>),
}
