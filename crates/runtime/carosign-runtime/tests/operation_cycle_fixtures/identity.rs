use super::*;

pub(super) struct Identity;

impl IdentityAuthority for Identity {
    fn resolve(
        &self,
        _caller: &CallerContext,
        _selector: &SubjectSelector,
    ) -> BoxFuture<'_, Result<SubjectRef, carosign_core::identity::IdentityError>> {
        Box::pin(async {
            Ok(SubjectRef {
                authority: AuthorityId::from_bytes([2u8; 16]).expect("валид"),
                subject: SubjectId::from_bytes([4u8; 16]).expect("валид"),
            })
        })
    }
}
