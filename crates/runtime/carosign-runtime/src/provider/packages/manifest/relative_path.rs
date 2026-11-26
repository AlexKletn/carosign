use super::ManifestError;

/// Path check: relative, no `..`No lead slash, no empty segments.
///
/// ⚠️ This is a non-existence test: the directory owner, not the manifesto, decides to trust
/// the package.
pub fn check_relative_path(path: &str) -> Result<(), ManifestError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\0')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(ManifestError::UnsafePath {
            path: path.to_string(),
        });
    }
    Ok(())
}
