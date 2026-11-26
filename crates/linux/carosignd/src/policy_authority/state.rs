use super::{GrantId, HashMap, Issued, PolicyChangeWatch};

pub(super) struct State {
    pub(super) watch: PolicyChangeWatch,
    pub(super) issued: HashMap<GrantId, Issued>,
    pub(super) invalid: bool,
}
