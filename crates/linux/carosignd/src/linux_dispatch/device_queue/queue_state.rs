use std::collections::VecDeque;

#[derive(Default)]
pub(super) struct QueueState {
    pub(super) active: bool,
    pub(super) waiting: VecDeque<u64>,
}
