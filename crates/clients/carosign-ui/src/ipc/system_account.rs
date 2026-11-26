use std::path::PathBuf;

pub(crate) struct SystemAccount {
    pub(crate) name: String,
    pub(crate) uid: u32,
    pub(crate) avatar: Option<PathBuf>,
}
