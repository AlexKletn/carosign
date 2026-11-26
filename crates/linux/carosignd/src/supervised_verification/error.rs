#[derive(Debug)]
pub enum Error {
    Context,
    Unavailable,
    Protocol,
    Timeout,
    Cleanup,
}
