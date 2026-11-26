use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use super::ProviderCatalog;

pub(crate) struct CatalogMonitor {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl CatalogMonitor {
    pub(crate) fn start(catalog: Arc<ProviderCatalog>) -> io::Result<Self> {
        let initial = catalog.publication_watch();
        let _ = catalog.inventory();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let worker = thread::Builder::new()
            .name("provider-catalog".into())
            .spawn(move || {
                let mut watch = initial;
                while !stopping.load(Ordering::Relaxed) {
                    let invalidate = match &watch {
                        Ok(watch) => match watch.changed(Duration::from_millis(100)) {
                            Ok(None) => continue,
                            Ok(Some(lost)) => lost,
                            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                            Err(_) => true,
                        },
                        Err(_) => {
                            thread::sleep(Duration::from_millis(100));
                            false
                        }
                    };
                    if invalidate {
                        catalog.invalidate_bindings();
                    }
                    watch = catalog.publication_watch();
                    let _ = catalog.inventory();
                }
            })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}

impl Drop for CatalogMonitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
