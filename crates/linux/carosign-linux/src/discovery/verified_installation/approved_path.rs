// Limit lengths refer to the installation record, not the kernel capabilities.
pub(super) fn approved_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part.len() <= 255
                && !part
                    .bytes()
                    .any(|byte| byte == 0 || byte.is_ascii_control())
        })
}
