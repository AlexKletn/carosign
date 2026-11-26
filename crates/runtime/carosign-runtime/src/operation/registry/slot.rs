use super::{Entry, Owner};

#[derive(Debug)]
pub(super) enum Slot {
    Present(Box<Entry>),
    CheckedOut,
    Recovering(Owner),
}
