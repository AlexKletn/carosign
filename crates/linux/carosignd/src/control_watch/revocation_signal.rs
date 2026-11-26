use std::sync::mpsc;

/// The only allowed entry is the source of politics.
pub(super) enum RevocationSignal {
    Revoked,
    ClientCancel(mpsc::SyncSender<bool>),
}
