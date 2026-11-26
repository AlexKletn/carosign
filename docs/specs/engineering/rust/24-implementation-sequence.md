# 24. Implementation work breakdown to module level

## Slice A — immutable foundation

Create core IDs, SubjectRef, AuthenticatorRef, operation contexts, result/evidence distinctions, limited capabilities; engine state transitions; stable error taxonomy. **Done when:** pure domain property tests and event replay pass without OS/DB.

## Slice B — persistent safety

Implement `StateStore` (initial redb adapter proposed), Registry CAS, journal prepare/dispatch/outcome/terminal and recovery scanner. **Done when:** process kill after every mutation barrier never loses uncertain scope or replays hardware mutation.

## Slice C — provider wire and host

Produce full normative `.proto` + seqpacket transport + Rust SDK minimal worker + independent C reference worker; Hello/Bind/Start/Cancel/Outcome; version negotiation, FD slots, unknown fields. **Done when:** both languages pass same black-box suite.

## Slice D — runtime orchestration

Operation actors, owner watchers, scheduler resource graph, provider manager/supervisor, event replay, evidence/authority ports. **Done when:** simulated worker performs verify/enroll/remove and survives crash/restart safely.

## Slice E — Linux product

`carosignd`, trusted inventory, process launch, optional broker, client IPC, `carosignctl`, PAM integration with password fallback; package metadata. **Done when:** separate install platform+test provider works without core change.

## Slice F — GXFP5130

Refactor device/capture/matcher/templates out of old `gxfp-authd` and build independent Rust provider; move policy/IPC out. **Done when:** full real sensor cycle, encrypted storage, user separation and cancellation.

## Slice G — libfprint bridge

C/GLib separate package + generated bindings; no Rust ABI. **Done when:** supported device works without changing platform and fprintd conflict handled.

## Slice H — Goodix QSEE

Native Rust worker, QSEE/listener integration, exact protocol and IRQ, opaque template/recovery. **Done when:** no stock Android HAL required for end-to-end loop, with safe failure semantics.

## Slice I — Veles/Nord4

External Veles integration project: Uzel, grants, subject map, Yav interaction, anchor/verifier, journaling. **Done when:** runtime contract consumed without Veles dependency in Core and guarantees are explicitly verified on real hardware.

## Cross-project definition of done

Each module has owner, public/internal interface, security boundary, deterministic tests where applicable, timeout/error/failure modes, and migration plan for stateful outputs. Avoid creating a new crate for each helper before a demonstrable reuse, independently versioned interface or feature-boundary reason exists.
