use super::ValidatedGrant;

pub(super) fn same_grant(a: &ValidatedGrant, b: &ValidatedGrant) -> bool {
    a.id() == b.id()
        && a.issuer() == b.issuer()
        && a.scope() == b.scope()
        && a.boot() == b.boot()
        && a.deadline_ns() == b.deadline_ns()
}
