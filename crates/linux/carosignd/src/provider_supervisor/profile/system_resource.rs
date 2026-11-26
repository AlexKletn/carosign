use super::PathBuf;

#[derive(Clone)]
pub(in super::super) struct SystemResource {
    pub(in super::super) path: PathBuf,
    pub(in super::super) role: carosign_protocol::carosign::provider::v1::FdRole,
    pub(in super::super) id: [u8; 16],
}
