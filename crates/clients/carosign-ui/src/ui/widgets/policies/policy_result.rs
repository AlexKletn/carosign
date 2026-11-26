use crate::ipc::SystemAccount;

pub(in crate::ui) enum PolicyResult {
    Loaded {
        document: String,
        accounts: Option<Vec<SystemAccount>>,
    },
    Saved(String),
    ServiceRestarted,
}
