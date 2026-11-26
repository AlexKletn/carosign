/// Short identifier form: the first four bytes.
///
/// ⚠️ For display only: the full identifier for such a label **not** is restored and is not
/// included in the record.
pub fn short(id: &[u8; 16]) -> String {
    id[..4].iter().map(|byte| format!("{byte:02x}")).collect()
}
