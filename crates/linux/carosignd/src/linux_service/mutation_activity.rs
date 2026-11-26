use super::ActiveMutation;

/// General accounting of changes for the coordinated diagnosis of the journal.
#[derive(Default)]
pub(crate) struct MutationActivity(pub(super) std::sync::Mutex<usize>);

impl MutationActivity {
    pub(crate) fn enter(&self) -> Result<ActiveMutation<'_>, ()> {
        let mut count = self.0.lock().map_err(|_| ())?;
        *count = count.checked_add(1).ok_or(())?;
        Ok(ActiveMutation(self))
    }
}
