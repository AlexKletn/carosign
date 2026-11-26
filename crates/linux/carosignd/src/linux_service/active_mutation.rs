use super::MutationActivity;

pub(crate) struct ActiveMutation<'a>(pub(super) &'a MutationActivity);

impl Drop for ActiveMutation<'_> {
    fn drop(&mut self) {
        if let Ok(mut count) = self.0.0.lock() {
            *count = count.saturating_sub(1);
        }
    }
}
