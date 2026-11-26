/// The key-value pair in the table.
pub type Entry = (Vec<u8>, Vec<u8>);
/// The contents of the shell vault tables.
pub type TableMap =
    std::collections::BTreeMap<&'static str, std::collections::BTreeMap<Vec<u8>, Vec<u8>>>;
