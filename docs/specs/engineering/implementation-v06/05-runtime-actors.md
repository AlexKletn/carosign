# 05. `carosign-runtime`: actor ownership, concurrency and effect execution

## Runtime topology

`RuntimeHandle` is cloneable narrow command sender; only builder wires trusted `AuthorityPort`, `ProviderExecutionPort`, `ResourcePort`, `EvidencePort`, `StateStore`, `TimeSource`, `InteractionPort`. Runtime does not import sockets, Goodix command numbers or fprintd types. Tokio is internal; `RuntimeHandle` async functions return domain types, not socket frames.

| Actor / service | Owns | Mailbox | State |
|---|---|---|---|
| OperationActor | one operation and domain state | bounded 64 messages | serialized decision transitions |
| OperationRegistry | `OperationId` → owner/handles/snapshots | bounded 256 | no I/O during map lock |
| ProviderManager | installed catalog, bind decisions | bounded 128 | never runs user-specified binary |
| WorkerSupervisor | process, pidfd, control channel | bounded 64/worker | worker epoch, crash handling |
| DeviceScheduler | resource conflict graph and queue | bounded 256 | exclusive claims, generation |
| RecoveryCoordinator | pending mutation scopes | bounded 64 | no automatic mutating replay |
| EventHub | per-op sequence, owner/observers | bounded 128 events/op | progress coalescing; terminal durable |

**Defaults** are implementation limits, not physiological timings. Configuration may reduce; raising requires memory-budget review. Each active operation owns at most one provider session and one resource lease set; no unbounded `tokio::spawn` per event. Reuse actor/task slots.

## Facade semantics

```rust
pub struct RuntimeHandle { /* private sender */ }
impl RuntimeHandle {
    pub async fn begin_verification(
        &self, caller: CallerContext, request: VerifyRequest,
    ) -> Result<OperationTicket, RuntimeError>;
    pub async fn begin_enrollment(
        &self, caller: CallerContext, request: EnrollRequest,
    ) -> Result<OperationTicket, RuntimeError>;
    pub async fn begin_removal(
        &self, caller: CallerContext, request: RemoveRequest,
    ) -> Result<OperationTicket, RuntimeError>;
    pub async fn cancel(
        &self, caller: CallerContext, id: OperationId,
    ) -> Result<CancelReceipt, RuntimeError>;
}
```

`CallerContext` is created **only** by trusted server adapter from peer credentials and connection binding; never deserialize it from public wire. `OperationTicket` contains the ID, deadline, current snapshot and an internal owner watcher handle, not a bearer token. Owner permissions are evaluated on every subsequent API call.

## Critical lock/await rules

1. Operation actor is the **only** writer of its `OperationState`. Other components send observations; no direct edits.
2. Never hold global operation registry, scheduler or DB locks across `await`, ioctl, FFI, process spawn or callbacks.
3. Reserve **canonical sorted ResourceSet** atomically. If unavailable, enqueue or return BUSY; do not hold a partial lease.
4. `DeviceEpoch` is fenced on every provider event and when releasing a lease. Old workers cannot release new ownership.
5. Worker control task must remain responsive during blocked hardware execution. Cancellation delivered via independent control path; if hardware call is not cancellable, move to `Uncertain` after bounded cleanup, not `Available`.
6. Capture progress is droppable/coalescible; mutation terminal commit/phase receipt is **not** droppable. If outgoing client buffer full, persist state and disconnect or send gap+snapshot, never block hardware actor on client write.
7. OwnerRevoked leads to suppression of later verification evidence delivery even if provider finishes successfully.
8. `Drop`/RAII may request best-effort cleanup, but is **not** the durable recovery mechanism.

## Effect execution and outbox

`EffectRunner` receives `Decision.effects`. For persistent mutation effects, first records effect intent/status in DB transaction (see [10](10-persistence-and-recovery.md)), then performs side-effect. For `SendMutatingCommand`, outbox status `DispatchArmed` must be durable **before** the first send; any ambiguous send result stays `NeedsReconciliation`. Completion callback feeds `DomainInput` back to actor, never changes state directly. Retry is allowed for explicitly idempotent **read-only** effects with bounded backoff; all other effects require typed recovery policy.

## Cancellation/lifetime rules

- `OwnerHandle`: connected to trusted caller + grant watcher; owner close/revoke triggers cancel for verification. Mutation continues as recovery obligation once dispatched.
- `ObserverHandle`: subscription only; disconnect does not cancel anything.
- Deadline: created from trusted `TimeSource`, based on boot-aware monotonic time. Across reboot the verification operation is invalid; mutation journal is durable, deadlines not reused.
- Provider termination: death of process != device quiesced. Supervisor orders Broker cleanup and records uncertain hardware until proven safe.
- Quiesce of underdisplay sensor includes terminating illumination and contact tracking under interaction lease owner.

## Startup (exact sequencing)

1. Load/validate strict system config, installed package catalog and trusted policy files.
2. Open metadata DB; verify schema/integrity. Do not initialize empty DB over corrupt DB.
3. Scan incomplete journals; mark affected resources/enrollments blocked before exposing the public listener.
4. Initialize authority, assessment, broker and clock/boot adapters.
5. Discover hardware, select approved provider, launch workers without conflicting mutations.
6. Attempt **safe read-only** reconciliation for each blocked scope; unknown stays blocked.
7. Publish IPC listener and health snapshot. Service may be `Degraded` with unavailable providers but must not report false ready for blocked scopes.

## Shutdown

Stop admission → revoke owner watchers → cancel verification → persist unresolved mutation phases → drain operations bounded → request worker quiesce → release broker resources → flush metadata → stop. Forced shutdown must leave durable indicators sufficient for recovery; cannot assume a TA command was rolled back.

### v0.6.4 — Runtime has no matching stage

Before `SendVerify`, OperationActor uses trusted Registry+Authority to create the exact current set of permitted enrollment bindings. Empty/invalid set fails without worker dispatch. The ProviderSession validates any `matched_enrollment` against that set; no-op progress is allowed. Runtime creates a ProviderAssertion evidence reference when Matched arrives with no independent token, and feeds the generic existing EvidencePort. Runtime never requests raw frames or evaluates scores/feature vectors/templates. See [29](29-v064-provider-owned-matching.md).

### v0.6.5 orchestration and non-blocking authority

Enrollment OperationActor manages the challenge/credential subphase as typed observations. Trusted credential UI/authority lives outside provider and Core, and cannot stall the ProviderSession reader or cancellation control path. HAT material is held only in the credential authority/trusted sink; actor carries refs. Active broker command bodies and DMA lifetime are owned by the trusted handler independently of actor cancellation. The logical immutable mutation plan is persisted before any pre-enroll preparation, and no post-arm worker retry is permitted. See [31–33](31-v065-enrollment-authorization.md).
