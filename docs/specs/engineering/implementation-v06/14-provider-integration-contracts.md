# 14. Concrete providers and external platform integrations

All providers are independently installable worker executables implementing `carosign.provider.v1`, never members of central Runtime crate. Provider-specific device protocol, format, keys and capture are private to provider.

## GXFP5130 / ChicagoHS

Separate workspace modules:

```
gxfp-hardware::{device,tls_psk,mailbox,irq,activation,calibration}
gxfp-capture::{stream,fdt,quality,burst,fusion}
gxfp-match::{ncc,phase,embedding,hybrid,score_policy}
gxfp-template::{set,serialization,format,storage_adapter}
gxfp-provider::{bootstrap,profile,commands,events,cancel,device_session}
```

GXFP remains host matching; `gxfp-match`'s score threshold and model selection are *provider-specific policy* and must not enter carosign-core. Raw 80×64 sensor frames never enter public service IPC. Software templates saved via protected store with explicit `format_id/version`, AEAD custody, consistent registry binding. Rust provider SDK optional but sensible. Existing GXFP `gxfp-authd` user management, PAM and secret emission should **not** be copied into new provider; these belong to platform/service or separate secret consumer. User-facing implementation in reference documents is evidence of existing approach, not proof of adequate security assurance.

## Goodix QSEE / TB371FC

Separate workspace:

```
qsee-transport::{scm,shared_buffers,session,errors}
qsee-listener-client::{filesystem,gpfs,rpmb,health}
goodix-wire::{commands,events,codec,validation}
goodix-device::{power,reset,irq,probe}
goodix-ta::{session,enroll,verify,delete,query}
goodix-orchestrator::{state_machine,cancel,recovery}
goodix-provider::{bootstrap,profile,commands,events}
```

Respect actual TA command ordering and opaque template custody. If `QueryMutationState` cannot independently confirm state, respond `Unknown`, not pretend success. No direct arbitrary raw QSEE/RPMB access to untrusted provider. Listener lifecycle separate from one biometric request. C prototype is a behavior reference, not architecture to port literally. Protocol numbers, IRQ behavior, cryptographic format are **hardware-specific** and must be taken from verified original research, not guessed by implementation agent.

## libfprint (C/GLib)

Package `carosign-provider-libfprint` uses public libfprint APIs and the exact wire ABI. `FpContext` discovery, `FpDevice` operations and `GCancellable` run under one long-lived GLib main context; cancellation ACK != quiesce. `FpPrint` (if serializable) stored by provider's protected template adapter with format/version and compatibility checks. The native fprintd service must not also own same physical sensor; broker enforces exclusivity. This bridge is **incoming hardware compatibility**; outgoing `fprintd` D-Bus facade is a different component. Python/Go providers pass same black-box conformance suite.

## Nord 4 / QTEE and FOD

Provider is independent; Veles-specific adapters remain **outside** platform workspace. `InteractionController` coordinates panel illumination, touch and sensor session through privileged trusted platform interfaces, with separate interaction lease. Provider does not get unrestricted UI/Wayland access. Verification proof acceptance via Gatekeeper/KeyMint requires actual compatible token chain. Research confirms some commands and enroll/match in isolated configurations; strict end-to-end claims **not** established. If unable to prove secure sensor path/attestation, mark assurance `Unverified`.

## Veles integration

`VelesBiometricAdapter` belongs to Veles repo, implements runtime ports for: identity/profiles, scoped grants/revocation, device registry/leases, Uzel transport, trusted UI/interaction, secure anchor verifier and journals reconciliation between profile binding and authenticator registry. Ownership: biometric runtime owns actual authenticator operations/records; Veles auth owns factor policy and profile binding. Prevent double-writer mutation across the boundary by a single coordinated operation ID and generation/fencing; do not start a second independent Veles mutation command behind platform.

## Agent STOP conditions

- Hardware command ordering/parameter not established by research → do not invent, record blocker.
- libfprint API unavailable on installed system → narrow build adapter/compatibility issue, do not modify core wire.
- Ambiguous provider device identity → don't auto-attach stored enrollments.
- Missing TPM/TEE verifier or insecure boot state → do not assert protected key result.
- Device cannot safely cancel/change state → return uncertain and block conflicting new mutations.

### Goodix reconciliation decision v0.6.1 (normative)

The research archive does **not** establish an independently verifiable, non-mutating Goodix TA `QueryMutationState` method. The baseline Goodix provider MUST advertise `reconciliation = Unavailable` until hardware evidence establishes a safe verifier. On query, return `QUERY_UNSUPPORTED` if no known supported query exists; return `QUERY_UNKNOWN` only when a supported safe query is attempted but cannot establish the required state. **Both** leave the affected mutation scope `NeedsReconciliation`, prevent further enrollment/removal there, and disable relying on that scope for verification until a trustworthy binding is recovered. `AuthenticatorReady`/successful initialization alone is never recovery evidence.

The only accepted paths to unblock are: (a) an independently verified TA/storage state-query mechanism documented with actual device evidence and adopted via ADR; (b) a documented authorized factory/reset/reprovision sequence proven to clear or invalidate the affected protected enrollment state, followed by independent verification and registry reconciliation; (c) disabling/decommissioning the affected authenticator without claiming successful recovery. Merely accepting an admin acknowledgement, reinstalling userspace, resetting the worker, or reading the lost worker log never proves hardware state. **No such query/reset path is claimed implemented in v0.6.1.** Implementation agent must return `ARCHITECTURE_BLOCKER` for production Goodix recovery rather than synthesize a query opcode. See [25](25-v061-conflict-resolutions.md).

### v0.6.4 — implementation strategies are completely provider-private

Match-on-Host, Match-on-Chip, TEE and libfprint drivers all implement the **same** StartVerification/Enrollment/Removal protocol. No shared matcher, frame pipeline, quality threshold or mandatory template export in Core or Runtime. Provider-managed storage may be entirely on-sensor/TA. A platform vault is an optional scoped encrypted blob facility, not a matching strategy. The worker MUST limit its match to Runtime-authorized bindings or refuse the request. See [29](29-v064-provider-owned-matching.md).

### v0.6.5 Goodix integration fences

Research establishes a Goodix pre-enroll challenge, Gatekeeper-generated HAT accepted by `gf_hal_enroll` to start capture, **not successful template persistence** (the observed run returned secure-object write failure 1035). Goodix enrollment must use the challenged token authority/sink contract [31](31-v065-enrollment-authorization.md), not diagnostic mode bypass. Safe no-persistent-side-effect behavior of `PRE_ENROLL` is provider-specific and must be verified before allowing the preparatory phase before arm. No generic signed HAT creator in Core.

Some Goodix typed command bodies are far larger than provider-control frame limit (research includes `0xc3568` and `0xee6a8` lengths). Transport via constrained broker sealed-memfd → vetted typed handler → DMA/kernel/TA is specified in [32](32-v065-bulk-command-buffer.md). The exact kernel module ABI, native_guard, status handling, DMA32 and trusted handler implementations are not provided here. Neither this contract nor the prototype C behavior authorizes deploying an arbitrary QSEE passthrough. Goodix incomplete enroll/recovery remains blocked until independent hardware-state evidence exists.
