/// Does the generation match what is expected?
///
/// ⚠️ The comparison is with the generation of the request, not the current one: “remove what
/// is now” is another operation and another risk.
pub fn generation_matches(expected: u64, current: u64) -> bool {
    expected == current
}
