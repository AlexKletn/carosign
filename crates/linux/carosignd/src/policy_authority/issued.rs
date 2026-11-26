use super::{CallerContext, ValidatedGrant};

pub(super) struct Issued {
    pub(super) caller: CallerContext,
    pub(super) grant: ValidatedGrant,
}
