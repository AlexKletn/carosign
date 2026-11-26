pub(in super::super) struct StagePermit<'a> {
    db: &'a redb::Database,
    id: [u8; 16],
}
impl StagePermit<'_> {
    pub(in super::super) fn matches(&self, db: &redb::Database, id: &[u8; 16]) -> bool {
        std::ptr::eq(self.db, db) && &self.id == id
    }
}
#[cfg(test)]
pub(in super::super) fn stage_fixture(db: &redb::Database, id: [u8; 16]) -> StagePermit<'_> {
    StagePermit { db, id }
}
