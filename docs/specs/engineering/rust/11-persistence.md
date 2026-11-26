# 11. `carosign-persistence`: Registry, journal, provider state

## Modules

```text
src/
  lib.rs
  ports/
    registry.rs            # EnrollmentRegistry
    journal.rs             # MutationJournal
    state_store.rs         # atomic metadata transaction
    provider_state.rs      # provider-owned opaque state catalog
  model/
    enrollment_record.rs
    mutation_record.rs
    provider_binding.rs
    schema_version.rs
  repository/
    redb.rs                # proposed first implementation
    transaction.rs         # critical commit groups
    migration.rs
    integrity.rs
  recovery/
    scan.rs
    scope_block.rs
    reconcile_plan.rs
  encryption/
    template_blob.rs       # only if platform-managed templates
    key_port.rs
    associated_data.rs
```

## Decision: one metadata transaction domain

Registry and MutationJournal remain logical ports, but **may share the same redb database** so `RegistryCommitted` and journal phase record can be committed atomically. This avoids unnecessary in-process consistency gaps. Hardware TA or provider-managed template store is *never* in this transaction. If separate databases are chosen later, replace with explicit durable cross-store protocol.

## Required interfaces

`StateStore::transaction(fn(tx) -> Result<...>)`; `RegistryRepository::list/get/apply_if_generation`; `MutationJournal::prepare/record_dispatch/record_outcome/mark_terminal/scan_incomplete`; `ProviderStateCatalog::lookup_binding/record_state_format`. Internal journal writes may have `DurabilityPolicy` but mutation dispatch only after `DurablePrepared` returned.

## Durable phases

`Prepared → DispatchPossible → ProviderObserved → RegistryCommitted → Completed`; exception `NeedsReconciliation`. `DispatchPossible` must be durably recorded before potentially mutating provider command is handed off. At recovery, if `DispatchPossible` exists, assume hardware may have changed. Do not rely solely on worker `MutationStarted`.

## Reconciliation

Load incomplete scopes before serving conflicting mutations; compare stored generation and actual provider/TA state using independent safe query if available. `Indeterminate` remains if unavailable. At reconciling, verify provider identity, device epoch and current identity authority before writing mapping. No blind repeat of mutation.

## Protected templates

Separate `TemplateRepository` belongs to provider or a delegated platform service. If platform-managed: AEAD with fresh nonce and AAD bound to subject/authenticator/enrollment/format/generation, plus key rotation and secure access. TPM-backed key retrieval is an adapter, not core. Software AEAD does not guarantee rollback prevention or trusted match.

## Storage schema / migrations

All records include schema version; migration path is transactional and recoverable. Store packages/profiles/enrollment references even if package removed. Do not garbage-collect templates on package uninstall. Detect corruption and fail closed rather than silently creating empty enrolment store.

## Persist/ack invariants

Notify committed mutation only *after* registry+journal durable terminal. External ack is separate and can be repeated without rerunning hardware. `Completed` does not imply that physical deletion erased all flash copies.
