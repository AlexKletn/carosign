pub(in super::super) struct SwitchPermit<'a> {
    db: &'a redb::Database,
    id: [u8; 16],
}

impl<'a> SwitchPermit<'a> {
    pub(in super::super) fn matches(&self, db: &redb::Database, id: &[u8; 16]) -> bool {
        std::ptr::eq(self.db, db) && &self.id == id
    }
}

#[cfg(test)]
pub(in super::super) fn fixture(db: &redb::Database, id: [u8; 16]) -> SwitchPermit<'_> {
    SwitchPermit { db, id }
}
