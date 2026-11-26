use std::collections::BTreeMap;

/// The tested composition is data only; it does not give permission for installation.
pub(in crate::discovery) struct CheckedArchive {
    pub(in crate::discovery) files: BTreeMap<String, Vec<u8>>,
}
