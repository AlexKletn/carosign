use std::sync::mpsc::SyncSender;
use std::time::Duration;

use crate::domain::Snapshot;

use super::read_snapshot::read_snapshot;

const INTERVAL: Duration = Duration::from_secs(3);

pub(crate) fn start(sender: SyncSender<Snapshot>, wake: impl Fn() -> bool + Send + 'static) {
    std::thread::spawn(move || {
        let mut previous = None;
        loop {
            let snapshot = read_snapshot();
            if previous.as_ref() != Some(&snapshot) {
                if sender.send(snapshot.clone()).is_err() || !wake() {
                    break;
                }
                previous = Some(snapshot);
            }
            std::thread::sleep(INTERVAL);
        }
    });
}
