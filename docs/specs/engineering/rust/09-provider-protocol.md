# 09. `carosign-protocol` и `carosign-provider-sdk`

## `carosign-protocol` modules

```text
src/
  lib.rs
  generated/
    client_v1.rs
    provider_v1.rs
  version.rs             # negotiated major/minor and features
  limits.rs              # decoded size/array/FD bounds
  schema.rs              # schema identity, reserved tags
```

Generated types are transport-neutral schema values, not domain decisions. `.proto` files under `protocols/` are intended to be canonical *after freeze*. Unknown security-sensitive enum variants reject, not silently downgrade. Protocol bindings are versioned separately from crate semver.

## `carosign-provider-sdk` modules

```text
src/
  lib.rs
  worker.rs              # public WorkerRunner facade
  handlers.rs            # provider operations trait (optional for Rust)
  channel.rs             # framed control/operation channel
  handshake.rs           # Hello negotiation, boot nonce/instance
  resources.rs           # validated delegated handles
  command.rs             # correlation, command receipt, outcome once
  event_sink.rs          # sequenced progress, terminal event
  cancellation.rs        # independent cancellation mailbox
  mapping.rs             # domain helpers ↔ wire
  shutdown.rs            # drain/quiesce/close
```

## SDK contract

Rust provider implements `ProviderHandler` convenience trait; C/Go/Python provider implements **identical wire protocol** with no SDK requirement. The SDK cannot request broader device permissions than manifest/system policy. No Rust trait object crosses the process boundary.

## Envelope and sequence rules

- One private inherited channel per worker instance; handshake must agree on protocol major, minor and features.
- Instance id, command id, operation id, device epoch and message sequence are distinct. Increment seq per channel; verify command correlation.
- `Accepted` is not completed; outcome is exactly once in a healthy exchange, late events rejected after terminal.
- After EOF, for mutations the runtime must assume command may have run once dispatch was possible.
- FD payloads have explicit slot descriptors/type/rights; reject missing/extra descriptors and truncation.
- Control packet size bounded (baseline proposal 64 KiB), large data via bounded typed FD or approved shared memory.

## Handshake mapping

`WorkerHello → Initialize → Initialized → BindDevice → DeviceReady`. No sensor operation before `DeviceReady`. Handler must not self-promote assurance; descriptor is capability declaration, assessment separate.

## SDK errors

`ProtocolViolation`, `WrongEpoch`, `UnsupportedFeature`, `InvalidCommand`, `OutOfResources`, `ProviderFailure`, `TransportClosed`, `CancelledRequested`, `HardwareStateUncertain` mapped to stable wire codes. Never rely on Rust string Debug as wire error text.

## Foreign language support

Maintain a canonical C minimal provider example and conformance fixture for each external language. No global Rust allocator in interface; cross-language lifetime is message/FD ownership, not borrowed Rust references.
