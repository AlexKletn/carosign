use std::path::PathBuf;

pub(super) struct PublishedDevice {
    pub(super) profile: Option<PathBuf>,
    pub(super) disabled: bool,
}
