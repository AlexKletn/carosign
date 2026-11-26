# 05. `carosign-runtime`: orchestration kernel

**Role:** executor-independent public surface wrapping async internal services. Runtime owns operation coordination and prevents provider-level quirks from reaching applications.

## Modules

```text
src/
  lib.rs
  builder.rs                 # RuntimeBuilder / required port validation
  handle.rs                  # RuntimeHandle: narrow command API
  services.rs                # owned services container, supervision tree
  operation_registry.rs      # operations map, owner/observer handles
  operation_actor.rs         # serialized domain transition + effect handling
  authz/                     # caller binding, grant validation, revocation
    mod.rs
    caller_binding.rs
    subject_resolver.rs
    grant_request.rs
    grant_validator.rs
    revocation.rs
    revalidate.rs
  operation_events.rs        # sequenced event log, bounded subscribers
  interaction.rs             # UX event routing + physical interaction port
  owner_watch.rs             # process/client/grant lifecycle
  timers.rs                  # deadlines via TimeSource port
  ids.rs                     # op IDs, instance tokens, boot identity
  effects.rs                 # dispatch engine Effect via ports
  ports/
    mod.rs
    provider.rs
    authority.rs
    resources.rs
    persistence.rs
    evidence.rs
    interaction.rs
    clock.rs
  providers/                 # see 06-provider-manager.md
  operations/                # see 07-operations.md
  scheduler/                 # see 08-scheduler.md
  recovery/
    bootstrap.rs
    reconcile.rs
    blocked_scopes.rs
  health.rs
  diagnostics.rs
  metrics.rs
  trace_context.rs
  privacy_filter.rs
  audit.rs
```

## Public facade

```rust
pub struct RuntimeBuilder { /* injected ports */ }
pub struct RuntimeHandle { /* opaque sender + state handle */ }
impl RuntimeHandle {
  pub async fn begin_verification(&self, caller: CallerContext, req: VerifyRequest) -> Result<OperationTicket, RuntimeError>;
  pub async fn begin_enrollment(&self, caller: CallerContext, req: EnrollRequest) -> Result<OperationTicket, RuntimeError>;
  pub async fn begin_removal(&self, caller: CallerContext, req: RemoveRequest) -> Result<OperationTicket, RuntimeError>;
  pub async fn cancel(&self, caller: CallerContext, operation: OperationId) -> Result<CancelReceipt, RuntimeError>;
}
```

Only runtime/platform adapter constructs trusted `CallerContext` from authenticated peer information. Caller-supplied bytes never instantiate trusted context directly.

## Concurrency/lock order

- One serial decision loop per `OperationId` (mailbox capacity bounded).
- Provider control channel independent of blocked hardware execution.
- No registry transaction or synchronous journal fsync while holding global operation registry lock.
- Scheduler resource set reservation is atomic and ordered; runtime never holds scheduler lock while awaiting provider response.
- `OwnerWatch` revocation interrupts operation via control mailbox; terminal delivery verifies owner/epoch/grant again.
- Backpressure policy: progress may be coalesced; terminal mutation state **never** silently dropped.

## Service startup ordering

1. Validate configuration and create trust boundaries.
2. Open persistent state and detect incomplete mutations.
3. Construct blocked scopes and boot identity.
4. Start authority/resource/evidence adapters.
5. Discover providers, but do **not** allow conflicting mutating operations yet.
6. Run reconciliation for affected scopes, exposing degraded state if unresolved.
7. Publish client listener / readiness once safe policy is established.

## Service shutdown

Reject new work → revoke owner handles → request cancellation → drain bounded interval → mark unresolved mutations → quiesce workers → close resource grants → flush state. Worker hang may require isolated process termination and hardware quarantine; never assert the device is free based solely on process exit.

## Resource limits

Service configuration supplies max active operations per caller, max queued per device, observer count, event buffer size, proof handle retention, and scheduling deadlines. All inputs are bounded before allocation.
