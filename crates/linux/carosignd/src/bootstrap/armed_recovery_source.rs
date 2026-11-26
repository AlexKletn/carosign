use carosign_persistence::{
    model::MutationRecord, mutation_store::RecoveryEvidence, recovery::MutationSnapshot,
};

pub trait ArmedRecoverySource {
    fn verify(&self, snapshot: &MutationSnapshot) -> Option<RecoveryEvidence>;
    fn confirm_cleanup(&self, mutation: &MutationRecord) -> bool;
}
