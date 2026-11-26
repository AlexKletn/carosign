pub(in super::super) struct PendingPermit<'a> {
    db: &'a redb::Database,
    journal: super::super::journal::Journal,
}
impl PendingPermit<'_> {
    pub(in super::super) fn journal_for(
        &self,
        db: &redb::Database,
    ) -> Option<super::super::journal::Journal> {
        std::ptr::eq(self.db, db).then_some(self.journal)
    }
}
#[cfg(test)]
pub(in super::super) fn pending_fixture(
    db: &redb::Database,
    journal: super::super::journal::Journal,
) -> PendingPermit<'_> {
    PendingPermit { db, journal }
}
