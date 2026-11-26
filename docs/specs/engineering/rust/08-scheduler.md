# 08. Scheduler, ресурсные аренды и fairness

## Modules (`carosign-runtime/src/scheduler`)

```text
scheduler/
  mod.rs
  resource_graph.rs          # physical/shared dependencies
  resource_set.rs            # canonical claims, conflict relation
  queue.rs                   # bounded per resource group
  priority.rs                # trusted role-based priority
  allocator.rs               # atomic reservation of ResourceSet
  lease.rs                   # lease ownership and fencing
  quiesce.rs                 # release verified vs revocation requested
  fairness.rs                # aging and starvation prevention
  recovery.rs                # Uncertain/Recovering quarantine slots
```

## Formal resource model

`ResourceId` names a physical/virtual exclusion unit. `ResourceClaim` describes access mode, sharing compatibility, scope, generation. `ResourceSet` is canonical and atomically reserved: if one claim cannot be granted, no partial lease escapes. Backend must implement consistent reservation or roll back uncommitted reservations before any worker receives them.

## Conflict predicate

`conflicts(a,b)` when they share a resource and modes are incompatible, OR one requires exclusive hardware state linked by an explicit dependency edge. Example: Goodix fingerprint sensor + QSEE exclusive session may overlap a shared listener but cannot concurrently execute two incompatible TA operations. `AuthenticatorId` alone is insufficient for conflict detection.

## Lease semantics

`LeaseGranted` permits scoped use; `Revoke` removes *future* permission, but a raw Linux FD may remain valid in worker. Physical reuse requires `Quiesced/Released` and, if applicable, reset/confirmation. `DeviceEpoch` increments for rebind/recovery; stale commands are fenced at protocol endpoints and resource broker.

## Fairness and denial of service

Bound per-caller queue and per-device operations. Trusted auth requests may have priority over diagnostic capture but no indefinite monopoly. Queue timeout, interaction timeout and hardware timeout are separate. Max in-flight, backpressure and cancellation are explicit policy.

## Lock/await rules

Do not hold allocator lock across IPC await, journal fsync or device call. Use pending reservation token, commit atomically, then lease active. Acquire resources in sorted order for multi-resource strategies. Lease release is idempotent by token but cannot falsify hardware state.

## Tests

Two clients same device cannot both be Active; two nonconflicting devices can run concurrently; reentrant cancel not free; crash maps to Uncertain; daemon reboot does not restore volatile lease as if still active; fairness under repeated high-priority requests.
