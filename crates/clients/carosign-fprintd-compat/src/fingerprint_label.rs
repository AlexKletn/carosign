pub(super) fn valid_label(label: &str) -> bool {
    matches!(
        label,
        "left-thumb"
            | "left-index-finger"
            | "left-middle-finger"
            | "left-ring-finger"
            | "left-little-finger"
            | "right-thumb"
            | "right-index-finger"
            | "right-middle-finger"
            | "right-ring-finger"
            | "right-little-finger"
    )
}
