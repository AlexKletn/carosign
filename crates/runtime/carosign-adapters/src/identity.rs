//! The explicit identity of the caller: uid directly, not by agreement on bytes.
//!
//! ⚠️ The uid used to come from the first bytes of the tag. `CallerContext`It was an
//! agreement, not a type, and anyone who changed the order of the label assembly broke the
//! authority. Now the uid is the **field of context, and the tag collector sets it
//! explicitly.
//!
//! ⚠️ Mark. `opaque` It's still there, and it's connected. It's not licensed anywhere else.

use carosign_core::ids::SubjectId;
use carosign_runtime::ports::CallerContext;

/// Who's here, clearly.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PeerIdentity {
    /// UID from core creeds.
    pub uid: u32,
    /// GID from core creeds.
    pub gid: u32,
    /// Start the process (to distinguish between reconnections of one uid).
    pub pid: u32,
}

impl PeerIdentity {
    /// Collects the context label and puts the uid in a separate field.
    ///
    /// ⚠️ Calls only transport from `SO_PEERCRED`You can't get in here from the wire.
    pub fn into_context(
        self,
        transaction: carosign_runtime::ports::TransactionId,
    ) -> CallerContext {
        let mut opaque = [0u8; 32];
        opaque[..4].copy_from_slice(&self.pid.to_be_bytes());
        opaque[4..8].copy_from_slice(&self.uid.to_be_bytes());
        opaque[8..12].copy_from_slice(&self.gid.to_be_bytes());
        opaque[31] = 1;
        CallerContext::with_uid(opaque, self.uid, transaction)
    }
}

/// The subject on the political number.
///
/// ⚠️ The number is not a name: the name shows the politician, and the platform does not
/// store it.
pub fn subject_id(number: u64) -> Option<SubjectId> {
    let mut bytes = [0u8; 16];
    bytes[8..].copy_from_slice(&number.to_be_bytes());
    bytes[0] = 1;
    SubjectId::from_bytes(bytes).ok()
}

/// Reverse: Number from the subject.
pub fn subject_number(subject: SubjectId) -> u64 {
    let bytes = subject.as_bytes();
    u64::from_be_bytes([
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    ])
}
