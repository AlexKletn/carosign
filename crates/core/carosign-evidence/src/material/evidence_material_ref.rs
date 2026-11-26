use super::EvidenceError;
use super::OwnerIdentity;
use carosign_engine::VerifiedState;
use core::fmt;
use core::hash::Hash;
use core::hash::Hasher;
use std::sync::Arc;

/// Reference to material withheld by delivery.
#[derive(Clone)]
pub struct EvidenceMaterialRef {
    id: [u8; 16],
    operation: [u8; 16],
    owner: Arc<OwnerIdentity>,
    generation: u64,
    digest: [u8; 32],
}

impl EvidenceMaterialRef {
    /// A reference to the material of this operation.
    ///
    /// ⚠️ It is going to ** only ** delivery: it cannot be obtained from the outside until
    /// the material is accepted.
    pub(crate) fn issued(
        id: [u8; 16],
        operation: [u8; 16],
        owner: Arc<OwnerIdentity>,
        generation: u64,
    ) -> Result<Self, EvidenceError> {
        if id == [0u8; 16] || operation == [0u8; 16] || generation == 0 {
            return Err(EvidenceError::MalformedEvidence);
        }
        Ok(Self {
            id,
            operation,
            owner,
            generation,
            digest: [0; 32],
        })
    }

    /// Material ID.
    pub fn id(&self) -> [u8; 16] {
        self.id
    }

    /// What kind of surgery you're attached to.
    pub fn operation(&self) -> [u8; 16] {
        self.operation
    }

    pub(crate) fn bind_digest(&mut self, digest: [u8; 32]) {
        self.digest = digest;
    }

    /// Verify the material without disclosing its contents.
    pub fn matches_digest(&self, digest: &[u8; 32]) -> bool {
        self.digest == *digest
    }
}

impl PartialEq for EvidenceMaterialRef {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.operation == other.operation
            && self.generation == other.generation
            && self.digest == other.digest
            && Arc::ptr_eq(&self.owner, &other.owner)
    }
}

impl Eq for EvidenceMaterialRef {}

impl Hash for EvidenceMaterialRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.operation.hash(state);
        self.generation.hash(state);
        self.digest.hash(state);
        Arc::as_ptr(&self.owner).hash(state);
    }
}

impl fmt::Debug for EvidenceMaterialRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EvidenceMaterialRef(..)")
    }
}

const _: () = {
    // `VerifiedState` It must be available from here: verification and verification of
    // evidence say the same thing.
    let _ = core::mem::size_of::<VerifiedState>();
};
