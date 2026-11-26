# 19. `carosign-provider-nord4-qtee` and external Veles adapter

Nord 4 is a **future provider** using stock trusted applications. The Veles trust-anchor document explicitly labels many parts as simulations / research; no claim of verified fingerprint → KeyMint end-to-end chain.

## Provider project

```text
providers/nord4-qtee/
  crates/
    qtee-client/            # QTEE object-reference transport
    nord4-fingerprint-ta/   # stock fingerprint TA protocol
    nord4-fod/              # touch + localized display illumination coordination
    nord4-provider/         # carosign.provider.v1 worker entry
  bins/
    bio-provider-nord4-qtee/
```

`qtee-client`: `open.rs`, `objects.rs`, `invoke.rs`, `listener.rs`, `errors.rs`. `fingerprint-ta`: `challenge.rs`, `gatekeeper_token.rs`, `enroll.rs`, `verify.rs`, `remove.rs`, `template_ref.rs`, `reconcile.rs`. `nord4-fod`: `geometry.rs`, `touch_lease.rs`, `illumination_lease.rs`, `timing.rs`, `cleanup.rs`. `provider`: `session.rs`, `capabilities.rs`, `events.rs`, `proof.rs`, `fault.rs`.

## System integration

`InteractionController` is injected. In Veles, Yav owns trusted presentation/interaction, device registry owns leases, `veles-auth` owns factors/profile policy, secrets service owns key operations. Provider coordinates but does not take over those responsibilities. In Linux, another adapter may offer limited FOD support with lower validated assurance.

## Evidence

Stock Gatekeeper auth token and fingerprint TA token are *different artifacts and roles*. Treat them as opaque until a compatible hardware verifier accepts actual nonce, profile, challenge, replay constraints and key operation. No self-asserted security level.

## Persistent mutations

Before enroll/remove dispatch, durable journal intent; if operation may have reached TA, cancellation/worker death requires reconciliation. Trusted storage listeners may outlive one fingerprint session, requiring separate liveness/health supervision. Device profile holds actual panel calibration and touch geometry, not core.

## External Veles integration project

```text
veles/workspaces/adapters/biometric/
  src/
    lib.rs
    runtime_bridge.rs
    subject_map.rs
    grant_adapter.rs
    resource_adapter.rs
    interaction_adapter.rs
    evidence_adapter.rs
    journal_link.rs
    uzel_server.rs
```

This code stays in Veles tree; it depends on independent semantic libraries and platform contracts, **not** vice versa. Prevent dual ownership: biometric runtime tracks authenticator state; Veles auth tracks factor-to-profile binding. Journal coordination across them requires a stable reconciliation protocol, not two unrelated reports of success.
