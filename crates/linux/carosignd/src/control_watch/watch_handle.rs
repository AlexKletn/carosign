use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// The flow stops and is expected upon explicit call or destruction.
pub(crate) struct WatchHandle {
    pub(super) stop: Arc<AtomicBool>,
    pub(super) thread: Option<std::thread::JoinHandle<()>>,
}

impl WatchHandle {
    pub fn stop(mut self) {
        self.finish();
    }
    pub fn wait(mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
    pub(crate) fn finish(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.finish();
    }
}
