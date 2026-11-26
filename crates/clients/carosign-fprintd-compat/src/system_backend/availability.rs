use std::time::{Duration, Instant};

// GNOME restarts its dialog when the reader type changes between NONE and PRESS.
const AVAILABILITY_REFRESH: Duration = Duration::from_millis(500);
const AVAILABILITY_GRACE: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(super) struct Availability {
    checked: Option<Instant>,
    last_success: Option<Instant>,
    pub(super) visible: bool,
}

impl Availability {
    pub(super) fn recently_checked(&self, now: Instant) -> bool {
        self.checked
            .is_some_and(|checked| now.duration_since(checked) < AVAILABILITY_REFRESH)
    }

    pub(super) fn record(&mut self, now: Instant, available: bool) -> bool {
        self.checked = Some(now);
        if available {
            self.last_success = Some(now);
        }
        self.visible = available
            || self
                .last_success
                .is_some_and(|last| now.duration_since(last) < AVAILABILITY_GRACE);
        self.visible
    }
}
