/// One rule of comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchRule {
    /// Sign (as the inventory calls it): `acpi`, `dt`, `usb`, `hid`).
    pub key: String,
    /// A meaning that should coincide entirely.
    pub value: String,
}
