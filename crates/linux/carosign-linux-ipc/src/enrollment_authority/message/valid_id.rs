pub(in crate::enrollment_authority) fn valid_id(value: &[u8]) -> bool {
    value.len() == 16 && value.iter().any(|v| *v != 0)
}
