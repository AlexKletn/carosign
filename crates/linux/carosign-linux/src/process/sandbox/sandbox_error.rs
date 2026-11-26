use core::fmt;

/// Sandbox violation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SandboxError {
    /// Root out.
    RootForbidden,
    /// There is no separate mounting space.
    MountNamespaceRequired,
    /// Network without permission.
    NetworkForbidden,
    /// Increased privilege.
    EscalationForbidden,
    /// New privileges.
    AmbientForbidden,
    /// Weakening for no reason.
    RelaxationWithoutReason {
        /// Value.
        field: &'static str,
    },
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootForbidden => f.write_str("работнику выдан корень"),
            Self::MountNamespaceRequired => {
                f.write_str("отдельное пространство монтирования обязательно")
            }
            Self::NetworkForbidden => f.write_str("сеть без отдельного разрешения"),
            Self::EscalationForbidden => f.write_str("повышение привилегий запрещено"),
            Self::AmbientForbidden => f.write_str("новые привилегии запрещены"),
            Self::RelaxationWithoutReason { field } => {
                write!(f, "ослабление {field} без причины")
            }
        }
    }
}

impl core::error::Error for SandboxError {}
