use super::{Allowance, Handler, PolicyError};

use carosign_core::ids::{PackageId, ResourceId};

use crate::resources::FdRole;

#[derive(Debug, Default)]
pub struct Policy {
    allowances: Vec<Allowance>,
    handlers: Vec<Handler>,
}

impl Policy {
    /// Empty politics: nothing is allowed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adding permission.
    pub fn allow(&mut self, allowance: Allowance) -> Result<(), PolicyError> {
        if self.allowances.contains(&allowance) {
            return Err(PolicyError::Duplicate);
        }
        self.allowances.push(allowance);
        Ok(())
    }

    /// Adds a handler.
    ///
    /// ⚠️ Without the handler, you can't: `invoke_any_ioctl`/`invoke_any_ta` It doesn't exist
    /// in any form.
    pub fn register(&mut self, handler: Handler) -> Result<(), PolicyError> {
        if handler.operation_ref == [0; 16] {
            return Err(PolicyError::Unsupported);
        }
        if self
            .handlers
            .iter()
            .any(|item| item.operation_ref == handler.operation_ref)
        {
            return Err(PolicyError::Duplicate);
        }
        self.handlers.push(handler);
        Ok(())
    }

    /// Is it allowed to give the resource of this role to this package?
    pub fn permitted(&self, package: PackageId, resource: ResourceId, role: FdRole) -> bool {
        self.allowances.iter().any(|allowance| {
            allowance.package == package
                && allowance.resource == resource
                && allowance.role == role
                && crate::resources::role_fits(allowance.role, allowance.kind)
        })
    }

    /// Finds a handler.
    pub fn handler(
        &self,
        operation_ref: [u8; 16],
        package: PackageId,
        resource: ResourceId,
    ) -> Result<&Handler, PolicyError> {
        self.handlers
            .iter()
            .find(|item| {
                item.operation_ref == operation_ref
                    && item.package == package
                    && item.resource == resource
            })
            .ok_or(PolicyError::Unsupported)
    }

    /// How many handlers.
    pub fn handlers(&self) -> usize {
        self.handlers.len()
    }
}
