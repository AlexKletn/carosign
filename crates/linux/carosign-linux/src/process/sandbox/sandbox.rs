use super::Relaxation;
use super::SandboxError;

/// That's allowed to an worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sandbox {
    pub network: bool,
    /// Your own mounting space.
    pub own_mount_namespace: bool,
    /// New privileges.
    pub ambient_capabilities: bool,
    /// Increased privileges (setuid bits and the like).
    pub may_escalate: bool,
    /// Getting the root.
    pub root: bool,
}

impl Default for Sandbox {
    fn default() -> Self {
        Self {
            network: false,
            own_mount_namespace: true,
            ambient_capabilities: false,
            may_escalate: false,
            root: false,
        }
    }
}

impl Sandbox {
    /// Checks the sandbox along with the allowed weakenings.
    ///
    /// ⚠️ The root is forbidden **always **; there is no weakening for it.
    pub fn check(&self, relaxations: &[Relaxation]) -> Result<(), SandboxError> {
        if self.root {
            return Err(SandboxError::RootForbidden);
        }
        if !self.own_mount_namespace {
            return Err(SandboxError::MountNamespaceRequired);
        }
        let allowed = |field: &'static str| {
            relaxations
                .iter()
                .any(|item| item.field == field && !item.reason.is_empty())
        };
        if self.network && !allowed("network") {
            return Err(SandboxError::NetworkForbidden);
        }
        if self.may_escalate {
            return Err(SandboxError::EscalationForbidden);
        }
        if self.ambient_capabilities {
            return Err(SandboxError::AmbientForbidden);
        }
        for item in relaxations {
            if item.reason.is_empty() {
                return Err(SandboxError::RelaxationWithoutReason { field: item.field });
            }
        }
        Ok(())
    }
}
