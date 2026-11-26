pub(super) fn restore_resource_owner(
    path: Option<&str>,
) -> std::io::Result<crate::resources::LocalResourceOwner> {
    Ok(match path {
        Some(path) => {
            use std::os::unix::fs::MetadataExt;
            let path = std::path::Path::new(path);
            if unsafe { libc::geteuid() } == 0
                && (crate::installation::trusted_store_path(path).is_err()
                    || !std::fs::symlink_metadata(path).is_ok_and(|metadata| {
                        metadata.file_type().is_file() && metadata.mode() & 0o077 == 0
                    }))
            {
                return Err(std::io::Error::other("хранилище ограждений не защищено"));
            }
            let store = carosign_persistence::redb::backend::InitialFenceStore::open_guarded(path)
                .map_err(|_| std::io::Error::other("хранилище ограждений недоступно"))?;
            crate::resources::LocalResourceOwner::from_initial_fence_store(store)
                .map_err(|_| std::io::Error::other("ограждения не восстановлены"))?
        }
        None => crate::resources::LocalResourceOwner::default(),
    })
}
