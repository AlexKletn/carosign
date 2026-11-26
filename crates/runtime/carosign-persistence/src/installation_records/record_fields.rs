pub(super) fn id(v: &[u8]) -> bool {
    v.len() == 16 && v.iter().any(|b| *b != 0)
}

pub(super) fn profile_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

pub(super) fn version(value: &str, allow_empty: bool) -> bool {
    (allow_empty || !value.is_empty())
        && value.len() <= 64
        && !value
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
}
