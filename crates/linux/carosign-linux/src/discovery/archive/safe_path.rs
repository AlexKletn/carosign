use super::ArchiveError;

pub(super) fn safe_path(raw: &[u8], directory: bool) -> Result<String, ArchiveError> {
    let text = std::str::from_utf8(raw).map_err(|_| ArchiveError::Unsafe)?;
    let text = if directory {
        text.strip_suffix('/').unwrap_or(text)
    } else {
        text
    };
    if text.is_empty()
        || text.len() > 1024
        || text.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.len() > 255
                || part.bytes().any(|b| b == 0 || b.is_ascii_control())
        })
    {
        return Err(ArchiveError::Unsafe);
    }
    Ok(text.to_owned())
}
