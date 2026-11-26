#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidRequest,
    MismatchedResponse,
    InvalidDescriptors,
}
