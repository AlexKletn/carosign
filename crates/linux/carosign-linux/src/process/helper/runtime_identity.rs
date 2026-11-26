/// Identity for which the passage to runtime files is prepared.
#[derive(Clone, Copy)]
pub(crate) struct RuntimeIdentity {
    pub(crate) uid: libc::uid_t,
    pub(crate) gid: libc::gid_t,
}
