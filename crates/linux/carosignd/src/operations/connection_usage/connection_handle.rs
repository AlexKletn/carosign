/// Internal non-renewable link to connection; not authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ConnectionHandle(pub(in super::super) u128);
