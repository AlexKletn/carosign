# 21. Errors, tracing and observability

## Cross-layer error taxonomy

| Layer | Error types | Meaning |
|---|---|---|
| IPC | `InvalidMessage`, `UnsupportedVersion`, `TooLarge`, `Unauthenticated` | malformed/unauthorized transport |
| Authority | `Denied`, `Revoked`, `StaleGrant` | policy rights |
| Scheduler | `Busy`, `WaitExpired`, `ResourceUncertain` | arbitration |
| Provider | `NoSample`, `NotMatched`, `SensorFault`, `TAUnavailable`, `Unsupported` | device/protocol |
| Mutation | `NoChange`, `Committed`, `NeedsReconciliation` | persistent state |
| Evidence | `Unverified`, `WrongChallenge`, `WrongRecipient`, `Expired`, `UnsupportedProof` | proof validity |
| Storage | `Corrupt`, `MigrationBlocked`, `SyncFailed`, `GenerationConflict` | persistence |

`NoMatch` is not an internal exception. `CommandAccepted` is not a successful operation. Unknown error codes do not turn into success.

## Runtime modules

`health.rs`, `diagnostics.rs`, `metrics.rs`, `trace_context.rs`, `privacy_filter.rs`, `audit.rs`. Correlation IDs: boot, provider instance, device epoch, operation ID (hashed in general logs if needed). Never record templates, raw frames, PSK, protected token, PIN/keyring secrets or non-redacted sample details.

## Health model

Expose separate `ServiceStatus`, `WorkerStatus`, `DeviceStatus`, `StorageStatus`, `TrustAssessmentStatus`. Worker alive does not mean hardware responsive. `Degraded` service may still support other devices. `Quarantined` must remain visible to administrator without automatically retrying mutation.

## Audit

Record authorization decision category, enrollment/remove mutation IDs, journal phase transitions, provider package version, rebind, quarantine, reconciliation result. Audit is not a replacement for durable mutation journal. Cap logs and mask personally sensitive fields.

## Timeouts

Separate policy timeouts: IPC handshake, queue wait, sensor finger wait, device call, cancel wait, worker shutdown, reconciliation. No universal five-second timeout that conflates API and hardware. Use monotonic clock in process; external deadlines invalid after boot change.
