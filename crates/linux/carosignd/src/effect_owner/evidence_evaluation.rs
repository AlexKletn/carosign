use carosign_engine::PlannedEffect;

#[derive(Clone)]
pub(super) struct EvidenceEvaluation {
    pub(super) effect: PlannedEffect,
    pub(super) material: carosign_evidence::material::EvidenceMaterialRef,
    pub(super) context: carosign_runtime::ports::EvidenceContext,
    pub(super) requirements: carosign_evidence::requirements::EvidenceRequirements,
    pub(super) result: Result<
        carosign_core::assurance::AssuranceAssessment,
        carosign_runtime::ports::EvidenceRefusal,
    >,
}
