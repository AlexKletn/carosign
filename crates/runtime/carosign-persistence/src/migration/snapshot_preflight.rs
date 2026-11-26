use super::{
    JOURNAL_KEY, MigratableRecord, PreflightFailure, PreflightSummary, ReadTransaction, Table,
    TableHandle, journal, prepare_v3_record,
};

/// Checks one v3 shot before preparing the migration, without a single entry in the database.
/// Verification is deliberately denied for species that do not yet have a rigorous
/// conversion; their contents are stored for subsequent sale.
#[allow(dead_code)]
pub(crate) fn preflight_v3_snapshot(
    tx: redb::ReadTransaction,
) -> Result<PreflightSummary, PreflightFailure> {
    preflight_v3_snapshot_inner(&tx, false)
}

/// Checks the same shot when recovering Pending, allowing only a verified log entry in
/// addition to the original metadate.
pub(super) fn preflight_v3_snapshot_inner(
    tx: &redb::ReadTransaction,
    pending: bool,
) -> Result<PreflightSummary, PreflightFailure> {
    const EXPECTED: &[&str] = &[
        "meta",
        "authenticators",
        "enrollments",
        "enrollment_index",
        "mutations",
        "mutation_scope_lock",
        "effect_outbox",
        "idempotency",
        "provider_state",
        "packages",
        "template_blobs",
    ];
    let tables = tx.list_tables().map_err(|_| PreflightFailure::Corrupt)?;
    for table in tables {
        if !EXPECTED.contains(&table.name()) {
            return Err(PreflightFailure::UnsupportedRecords);
        }
    }
    if tx
        .list_multimap_tables()
        .map_err(|_| PreflightFailure::Corrupt)?
        .next()
        .is_some()
    {
        return Err(PreflightFailure::UnsupportedRecords);
    }
    let packages = crate::installation_records::preflight_v3_packages(tx)
        .map_err(|_| PreflightFailure::Corrupt)?;
    let tx = crate::redb::backend::ReadTxRef { tx };
    let state = crate::recovery::StartupScan::scan_v3_snapshot(&tx)
        .map_err(|_| PreflightFailure::Corrupt)?;
    if !state.blocked.is_empty() {
        return Err(PreflightFailure::ActiveMutation);
    }
    for table in [
        Table::Authenticators,
        Table::ProviderState,
        Table::TemplateBlobs,
    ] {
        if !tx
            .scan_prefix(table, &[])
            .map_err(|_| PreflightFailure::Corrupt)?
            .is_empty()
        {
            return Err(PreflightFailure::UnsupportedRecords);
        }
    }
    let meta = tx
        .scan_prefix(Table::Meta, &[])
        .map_err(|_| PreflightFailure::Corrupt)?;
    let valid_schema = meta.iter().any(|(key, value)| {
        key == crate::redb::backend::SCHEMA_KEY && value == crate::redb::backend::SCHEMA_V3
    });
    let valid_journal = meta.iter().any(|(key, value)| {
        key == JOURNAL_KEY
            && journal::Journal::decode(value)
                .is_ok_and(|record| record.stage == journal::Stage::Pending)
    });
    if !valid_schema
        || if pending {
            meta.len() != 2 || !valid_journal
        } else {
            meta.len() != 1
        }
    {
        return Err(PreflightFailure::UnsupportedRecords);
    }
    let mut records = 0usize;
    for (table, kind) in [
        (Table::Enrollments, MigratableRecord::Enrollment),
        (Table::Mutations, MigratableRecord::Mutation),
        (Table::EffectOutbox, MigratableRecord::Outbox),
        (Table::Idempotency, MigratableRecord::Idempotency),
    ] {
        for (key, value) in tx
            .scan_prefix(table, &[])
            .map_err(|_| PreflightFailure::Corrupt)?
        {
            prepare_v3_record(kind, &key, &value).map_err(|_| PreflightFailure::Corrupt)?;
            records = records.checked_add(1).ok_or(PreflightFailure::Corrupt)?;
        }
    }
    Ok(PreflightSummary { packages, records })
}
