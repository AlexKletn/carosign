use super::InstanceModel;

/// Section `[worker]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerSection {
    /// Relative path.
    pub executable: String,
    /// Model specimen.
    pub instance_model: InstanceModel,
}
