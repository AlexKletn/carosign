use super::Backend;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    Verify,
    Enroll,
    Delete,
}

pub(super) fn stop_action(backend: &mut dyn Backend, caller: &str, action: Option<Action>) {
    match action {
        Some(Action::Verify) => backend.stop(caller),
        Some(Action::Enroll) => backend.stop_enroll(caller),
        Some(Action::Delete) => backend.stop_delete(caller),
        None => {}
    }
}
