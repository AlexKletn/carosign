use carosign_adapters::evidence::EvidenceCheck;
use carosign_evidence::verifier::{Observed, Verdict};

pub(super) struct Never;

impl EvidenceCheck for Never {
    fn check(
        &self,
        _: carosign_evidence::material::EvidenceMaterialRef,
        _: &carosign_runtime::ports::EvidenceContext,
        _: &carosign_evidence::requirements::EvidenceRequirements,
    ) -> Verdict {
        panic!("несовпадающий контекст не должен достигать проверяющего")
    }
    fn observed(&self) -> Observed {
        panic!("наблюдение не запрашивается")
    }
}
