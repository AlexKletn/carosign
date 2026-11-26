/// What happened to the material?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MaterialState {
    /// Not yet charged.
    Held,
    /// Presented; not reusable.
    Consumed,
    Destroyed,
}
