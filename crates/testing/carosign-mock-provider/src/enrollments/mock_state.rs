use super::mock_record::MockRecord;

#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct MockState {
    #[prost(message, repeated, tag = "1")]
    pub(super) records: Vec<MockRecord>,
}
