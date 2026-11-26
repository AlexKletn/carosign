# 10. `carosign-linux-ipc`: public protocol and Linux transport

## Modules

```text
src/
  lib.rs
  transport/
    seqpacket.rs          # syscalls / message boundary checks
    peer.rs               # credentials / process lifetime
    ancillary.rs          # SCM_RIGHTS validator
    limits.rs             # per-peer size/FD/counter caps
  server/
    listener.rs
    connection.rs          # Hello and lifecycle
    dispatch.rs            # API request to RuntimeHandle
    subscriptions.rs       # observer replay, backpressure
    authz_context.rs       # trusted caller from OS credentials
  client/
    connection.rs
    requests.rs
    observe.rs
    reconnect.rs
  mapping/
    requests.rs
    results.rs
    errors.rs
```

## API surface

`Hello; GetDevices; GetCapabilities; ListEnrollments; BeginVerification; BeginEnrollment; BeginRemoval; GetOperation; ObserveOperation; CancelOperation; GetHealth` from v0.4. `OperationAccepted` is acknowledgement of scheduling only. `OwnerHandle` has authority and lifetime tied to trusted connection+grant; `ObserverHandle` is separate.

## Client identity

Use kernel-provided connection credentials as *input* to policy adapter; they are not authorization by themselves. Some privileged services proxy requests, so a system-defined delegation must be verified. Plain `user` field cannot target arbitrary subject. Client-provided idempotency key is scoped to trusted caller/subject/operation fingerprint.

## Wire/FD rules

`AF_UNIX SOCK_SEQPACKET` and Protobuf are baseline proposals, not requirements for Veles. Reject truncated packet (`MSG_TRUNC`) and truncated cmsg (`MSG_CTRUNC`), unexpected FD slots, excessive lengths and invalid flags. Do not allow raw template bytes in public API. For `SCM_RIGHTS`, confirm file-type/permissions and close unconsumed descriptors.

## Event semantics

Per-operation monotonically increasing sequence; bounded progress history; terminal mutation result persisted until ack/retention policy. Reconnect request with older sequence may get `Gap + Snapshot`. Provider event sequence is not public sequence: server filters, normalizes and assigns its own.

## Public security

Unknown client protobuf fields must not silently authorize new meanings. Version negotiation applies capabilities. Restrict `GetHealth` to sanitized status; diagnostic trace and raw frames are separate privileged API, off by default. fprintd-compatible D-Bus bridge has no authority to bypass new enrollment policy.

## Tests

Connect/disconnect, version negotiation, truncated ancillary data, FD spoofing, parser fuzzing, multiple observers, slow-reader backlog, cross-UID access, client death during verification and mutation.
