use super::InvalidSubject;
use crate::ids::AuthorityId;
use crate::ids::SubjectId;
use core::fmt;

/// Length of the subject's canonical record: `authority[16] || subject[16]`.
pub const SUBJECT_REF_LEN: usize = 32;

/// Length of domain prefix: subject plus authenticator, `32 + 16`.
pub const SUBJECT_SCOPE_PREFIX_LEN: usize = 48;

/// Who owns the registrations, a pair, not one field.
///
/// ⚠️ We don't. `PartialEq`/`Hash` manually: the derivative gives exactly what you want,
/// comparing both fields in order of announcement. `SubjectId` (see document 03, “Canonical
/// subject comparison”).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubjectRef {
    /// Who gave up the unsub.
    pub authority: AuthorityId,
    /// The subject inside the source.
    pub subject: SubjectId,
}

impl SubjectRef {
    /// Canonical entry: `authority` later `subject`, exactly 32 bytes.
    ///
    /// This entry goes first to the keys. `enrollment_index` and `mutation_scope_lock`
    /// (Document 23), so the layout must be one for all.
    pub fn to_canonical(self) -> [u8; SUBJECT_REF_LEN] {
        let mut out = [0u8; SUBJECT_REF_LEN];
        out[..16].copy_from_slice(self.authority.as_bytes());
        out[16..].copy_from_slice(self.subject.as_bytes());
        out
    }

    /// Takes apart the canonical record back in pair.
    ///
    /// ⚠️ The analysis of **syntactic **: the resulting pair does not mean that such a
    /// subject exists.`IdentityAuthority`](crate::identity::IdentityAuthority).
    pub fn from_canonical(bytes: &[u8]) -> Result<Self, InvalidSubject> {
        let bytes: [u8; SUBJECT_REF_LEN] = bytes.try_into().map_err(|_| InvalidSubject)?;
        let authority = AuthorityId::try_from(&bytes[..16]).map_err(|_| InvalidSubject)?;
        let subject = SubjectId::try_from(&bytes[16..]).map_err(|_| InvalidSubject)?;
        Ok(Self { authority, subject })
    }
}

impl fmt::Debug for SubjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The content is not included in the log: it is the identity of the user.
        f.write_str("SubjectRef(..)")
    }
}
