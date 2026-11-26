use super::{Installed, ManagerError, PACKAGES_MAX};
use crate::ports::QuiesceAssessment;
use crate::provider::inventory::Inventory;
use crate::provider::packages::{Candidate, Profile, Resolution};
use crate::provider::supervisor::Supervisor;

/// Package catalogue.
#[derive(Debug, Default)]
pub struct ProviderManager {
    installed: Vec<Installed>,
    preferred: Vec<String>,
    supervisor: Supervisor,
}

impl ProviderManager {
    /// Empty catalogue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an installed package.
    ///
    /// ⚠️ Repeating the packet name is a failure, not a replacement: otherwise, you can
    /// replace the provider behind your back with an update.
    pub fn install(&mut self, package: Installed) -> Result<(), ManagerError> {
        if !package.trusted {
            // ⚠️ An untrusted package is not considered at all.
            return Err(ManagerError::Untrusted);
        }
        if self.installed.iter().any(|item| {
            item.manifest
                .package
                .id
                .eq_ignore_ascii_case(&package.manifest.package.id)
        }) {
            return Err(ManagerError::AlreadyInstalled);
        }
        if self.installed.len() >= PACKAGES_MAX {
            return Err(ManagerError::TooMany);
        }
        self.installed.push(package);
        Ok(())
    }

    /// Takes the package out of consideration.
    ///
    /// ⚠️ Registrations **not ** are deleted (document 12).
    pub fn uninstall(&mut self, package: &str) -> bool {
        let before = self.installed.len();
        self.installed
            .retain(|item| item.manifest.package.id != package);
        before != self.installed.len()
    }

    /// Set the priority of packages (policy, not manifesto).
    pub fn set_preferred(&mut self, order: Vec<String>) {
        self.preferred = order;
    }

    /// How many are set.
    pub fn len(&self) -> usize {
        self.installed.len()
    }

    /// Nothing is established - fine.
    pub fn is_empty(&self) -> bool {
        self.installed.is_empty()
    }

    /// Selects a provider for inventory.
    ///
    /// Selection uses inventory metadata. The package runs only after selection.
    pub fn resolve(
        &self,
        inventory: &Inventory,
        admin_choice: Option<&str>,
    ) -> Vec<(String, Resolution)> {
        let mut out = Vec::new();
        let pairs: Vec<(String, Profile)> = self
            .installed
            .iter()
            // An incomplete or contradictory set of profiles does not allow the package to be
            // considered in part.
            .filter(|item| {
                item.manifest.profiles.len() == item.profiles.len()
                    && item.manifest.profiles.iter().all(|entry| {
                        item.manifest
                            .profiles
                            .iter()
                            .filter(|other| other.id == entry.id)
                            .count()
                            == 1
                            && item
                                .profiles
                                .iter()
                                .filter(|profile| profile.id == entry.id)
                                .count()
                                == 1
                    })
            })
            .flat_map(|item| {
                item.profiles
                    .iter()
                    .map(move |profile| (item.manifest.package.id.clone(), profile.clone()))
            })
            .collect();

        for entry in inventory.entries() {
            let resolution = crate::provider::packages::resolve(
                &pairs,
                &entry.signals,
                &self.preferred,
                admin_choice,
            );
            out.push((entry.locator.as_str().to_string(), resolution));
        }
        out
    }

    /// The candidate is selected if he is the only one allowed.
    pub fn chosen(&self, inventory: &Inventory) -> Option<Candidate> {
        let resolved = self.resolve(inventory, None);
        let mut chosen = None;
        for (_, resolution) in resolved {
            match resolution {
                Resolution::Chosen(candidate) => {
                    if chosen.is_some() {
                        // The solution is not the only one – we do not choose at all.
                        return None;
                    }
                    chosen = Some(candidate);
                }
                _ => return None,
            }
        }
        chosen
    }

    /// An worker's supervisor.
    pub fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }

    /// Supervisor for editing (start, loss).
    pub fn supervisor_mut(&mut self) -> &mut Supervisor {
        &mut self.supervisor
    }

    /// What to say about cleaning after a loss.
    pub fn after_loss(&mut self) -> QuiesceAssessment {
        match self.supervisor.lost() {
            Ok((_, assessment)) => assessment,
            Err(_) => QuiesceAssessment::Uncertain,
        }
    }
}
