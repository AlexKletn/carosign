# 25. v0.6.1 — normative reconciliation of four review findings

**Status:** decision supplement to the v0.6 implementation baseline. These decisions **supersede ambiguous text** in documents 03, 09, 10, 14, 15, 17, 20, 21, 23, 24. No new hardware behavior is asserted. The external wire protocols remain not publicly frozen until conformance.

## R-01: `template_blobs` primary key and mirror

Canonical redb key = exactly 24 bytes: `EnrollmentId[16] || generation.to_be_bytes()[8]`. `ProtectedTemplateRecord.enrollment_id` and `.generation` inside `StoredValue.template_blob` are deliberate, redundant **mirrors used to validate record identity**, not a second key or a competing source of truth. Writer derives mirrors from validated key; reader compares both byte-for-byte before AEAD decryption, otherwise `CorruptRecord` and fail closed. Generation >0, ID nonzero. Registry binding and AAD must also agree in one consistent snapshot. Both key encoding and value layout are normative; **do not remove fields without storage schema migration**. Tests D-11/D-12.

## R-02: FD roles, broker binding, mediated service

The only `SCM_RIGHTS` carrier in `carosign.provider.v1` minor 0 is `BindAuthenticator`; `FD_ROLE_MEDIATED_SERVICE` is specifically a broker-connected `AF_UNIX/SOCK_SEQPACKET` FD. That socket speaks separate [`carosign.broker.v1`](26-mediated-broker-v1.md). It does not grant arbitrary hardware operations. Each mediated Call is checked by broker against an allowlisted, typed, installed handler, worker, lease, resource, epoch, policy, and bounded request schema. Handler availability is provider/platform-specific; no invented QSEE commands. `FdSlot.resource_id` is a nonzero 16B ResourceId from broker's *exact* active lease, not a provider-created ID; validate all roles, indices 0..N−1, count, duplicates, actual FD identities before granting hardware access. No other FD carrier or FD-passing stream is defined in minor 0. Tests P-13/P-14/P-15.

## R-03: Goodix reconciliation when no safe TA query exists

The current source material does **not establish** a trustworthy non-mutating TA-state query or reset protocol for the Goodix/TB371FC hardware. The Goodix provider advertises no verified `SAFE_MUTATION_QUERY` capability in baseline. A `QueryMutation` request without capability produces `QUERY_UNSUPPORTED`; an inconclusive attempt with a separately proven safe query yields `QUERY_UNKNOWN`. Both leave affected scope `NeedsReconciliation`, its mutation lock held, and verification depending on affected enrollment **disabled**. Worker restart, reconnect, administrator click, or repeated command does **not** resolve this. Unblocking requires independently verified actual TA state, or a proven authorized destructive reprovision/clear plus independent validation; otherwise the device/scope remains disabled or is decommissioned. The source does not prove that either remediation exists. Coding agent reports `ARCHITECTURE_BLOCKER` for real Goodix recovery. Tests G-01/G-02/G-03.

## R-04: Composite subject identity and indexes

`SubjectRef` = `(AuthorityId, SubjectId)`. Equality and Hash MUST include **both** newtyped 16-byte components. Canonical binary encoding = `AuthorityId[16] || SubjectId[16]`. `enrollment_index` key is `AuthorityId[16] || SubjectId[16] || AuthenticatorId[16] || EnrollmentId[16]` (64 bytes), and scope prefix is first 48 bytes. `mutation_scope_lock` also uses the 48-byte triple, never SubjectId alone. No identity collision when two authorities choose the same SubjectId. Same reference must be resolved through trusted `IdentityAuthority`, not accepted as authority from client wire. Tests C-02/C-02b.

## Resolution precedence

This file, the amended normative implementation-v06 documents, and checked-in schemas form v0.6.1 baseline. Older v0.5 and earlier v0.6 suggestions are not alternative implementation choices. If implementation requires different semantics, stop and request ADR; do not silently choose. Hardware-specific unknowns remain STOP conditions, not permission to infer capabilities.

**Mediated mutation fence:** each persistently mutating `carosign.broker.v1.Call` requires operation/mutation/command IDs and an independent trusted lookup of the already-durable `DispatchArmed` record before invocation; the worker claim is not trusted. Multiple mutating calls for one armed command are prohibited unless a trusted audited handler encloses the entire command. See [26](26-mediated-broker-v1.md).

### v0.6.5 historical version note

This section accurately documents provider protocol **minor 0** at the time. Current provider-control protocol remains `BindAuthenticator`-only FD passing even at minor 2. A separate **broker** protocol minor 1 permits a single sealed memfd with `Call`/`CallResult.external_body`, subject to [32](32-v065-bulk-command-buffer.md). This does not undo the earlier device-FD authority restrictions.

**v0.6.6 provider minor note:** the prior `minor 2` statement in this historical review is superseded. Live provider protocol requires minor **3** for explicit planned enrollment identity; provider-control FD restriction stays unchanged. [34](34-v066-enrollment-command-projection.md).
