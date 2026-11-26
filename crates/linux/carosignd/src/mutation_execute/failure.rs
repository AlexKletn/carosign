#[derive(Debug)]
pub(crate) enum MutationStartError {
    Context,
    Authority,
    Storage,
    Resource,
    Provider,
    State,
}
