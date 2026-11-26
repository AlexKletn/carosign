pub(super) fn publish_client_socket(path: &str) -> std::io::Result<std::os::fd::OwnedFd> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
    let path = std::path::Path::new(path);
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("нет каталога сокета"))?;
    let directory_fd = std::fs::File::open(parent)?;
    let directory = directory_fd.metadata()?;
    // The directory should exclude the name substitution between bind and chmod.
    if !directory.is_dir()
        || directory.uid() != unsafe { libc::geteuid() }
        || directory.mode() & 0o022 != 0
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "незащищённый каталог сокета",
        ));
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| std::io::Error::other("неверное имя сокета"))?;
    let anchored = format!("/proc/self/fd/{}/{name}", directory_fd.as_raw_fd());
    let listener = carosign_linux::sys::listen_unix(&anchored)
        .map_err(|error| std::io::Error::from_raw_os_error(error.errno))?;
    let entry = std::fs::symlink_metadata(&anchored)?;
    if !entry.file_type().is_socket() || entry.uid() != directory.uid() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "неверный владелец сокета",
        ));
    }
    // Connection is not a right to an operation: the authority is verified by the service.
    std::fs::set_permissions(&anchored, std::fs::Permissions::from_mode(0o666))?;
    Ok(listener)
}
