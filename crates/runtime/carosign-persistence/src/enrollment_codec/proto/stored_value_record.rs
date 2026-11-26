use super::stored_value_kind as stored_value;

/// Value with the schema version.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoredValue {
    /// A version of the circuit.
    #[prost(uint32, tag = "1")]
    pub schema_version: u32,
    /// What's inside.
    #[prost(oneof = "stored_value::Value", tags = "11")]
    pub value: Option<stored_value::Value>,
}
