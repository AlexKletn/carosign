# 25. Module-level flows, errors and handoffs

## Worker boot (private channel)

```text
carosignd::bootstrap
  → runtime::providers::discovery
  → runtime::providers::resolver
  → linux::process::spawn
  → provider_sdk::handshake (remote worker may be any language)
  → runtime::scheduler::allocator
  → linux::resources::lease
  → provider::BindDevice
  → runtime::providers::instance_registry
  → runtime::health::publish_authenticator
```

On any failure, release unpublished resources; do not publish `Ready` before DeviceReady and compatibility checks. A worker whose process starts successfully is not automatically a usable authenticator.

## Verification via PAM

```text
pam::client → linux_ipc::server::dispatch
  → runtime::authz::grant_request
  → runtime::operation_registry
  → runtime::scheduler::allocator
  → runtime::operations::verify
  → provider_sdk::handlers::verify (other process)
  → runtime::operations::observations
  → evidence::verifier + authority revalidation
  → runtime::operations::completion
  → linux_ipc::server::subscriptions
  → pam::result_map
```

Provider says `Matched`; evidence/authority can still reject. Token cannot be replayed after operation invalidation.

## Enroll + durable mutation

```text
client → runtime::authz
  → persistence::journal::prepare (durable)
  → scheduler::allocator → grant revalidate
  → persistence::journal::record_dispatch (durable)
  → provider::StartEnrollment / Commit
  → provider::Outcome / provider-specific state query
  → persistence::transaction::commit_registry_and_journal
  → operation event terminal + ack
```

If crash after dispatch, recover with blocked scope and independent state query. Never blindly send `Enroll` again.

## Cancellation while blocked in hardware

```text
public CancelOperation
  → runtime::operation_registry::owner_check
  → operation actor records CancellationRequested
  → provider control channel sends RequestCancel
  → worker control task tries hardware cancellation
  → supervisor waits for Quiesced or isolates
  → scheduler releases only after physical proof/recovery
```

Cancellation acknowledgement means requested, not done.

## Veles

```text
veles-auth → external Uzel adapter
  → independent RuntimeHandle
  → Provider Worker via common protocol
  → external Veles resource/interaction/authority ports
```

`veles-auth` remains policy owner, Yav remains presentation owner and Veles secrets service remains key owner. No dependency from independent Core to Veles packages.

## libfprint

```text
carosignd → provider wire → C/GLib libfprint worker
  → FpContext/FpDevice/FpPrint
  → provider wire result → biometric runtime
```

No direct libfprint dependency in Rust Core. API fprintd compatibility is a distinct outbound service.
