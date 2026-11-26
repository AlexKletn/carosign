# 16. Work orders for implementation agent (architecture locked)

**Instructions for agent:** work in exact order. Do not implement future slice before prior gates pass. Do not invent hardware knowledge or adjust ABI/DB schema. Each work order is a bounded deliverable with files, tests and acceptance. If requirement conflicts with installed dependencies or actual hardware, output `ARCHITECTURE_BLOCKER` rather than improvise. For each change provide list of modified files, tests run, result, and unresolved blockers. **Do not claim tests not executed.**

## W00 — scaffold (no behavior)

Create workspace, crates and module files **exactly** according to [02](02-crates-and-modules.md). Choose Rust 2024 edition as baseline. Add `#![forbid(unsafe_code)]` core and engine. Add empty facade compile-only where required. Do not add provider code to main Cargo workspace. Acceptance: workspace resolves local crate DAG, no cycles; compile when toolchain exists. **No new dependencies not justified by owner module.**

## W01 — typed core IDs and values

Files `carosign-core/src/{ids,identity,device,enrollment,operation/*,limits,error}.rs`. Implement 16-byte newtypes, fixed serialization adapters, generation/epoch checked math, SubjectRef, requests, domain errors. Acceptance C-01..C-03, no system APIs or `serde` wire DTO exposure into domain. Reject invalid labels and opaque reference lengths.

## W02 — pure engine

Files `carosign-engine/src/{input,effect,decision,transition,invariant,machines/*,recovery,replay}.rs`. Implement [04](04-pure-engine.md) transition tables, effect ID tuple, pure observations and effect receipts. No I/O or Tokio. Acceptance E-01..E-07 and B-01a..B-01f / B-02d..B-02e; no mutation dispatch effect before durable arm observation. Explicitly implement both verified reconciliation outcomes through `Committing`, no direct `Terminal`, and distinct verification send failures.

## W03 — persistence & outbox

Files `carosign-persistence/src/{store,transaction,registry,journal,outbox,redb/*,recovery/*}` (per tree). Implement redb schema exactly [10](10-persistence-and-recovery.md), CAS, rollback-safe staging, incomplete scan. Use versioned durable value envelopes. Acceptance D-01..D-08 and B-02a..B-02c/B-02f with kill/fault injection; explicit `terminal_outcome` persisted, no blind retry of armed mutating commands.

## W04 — protocol schemas and validation

Use actual `.proto` files in `contracts/proto/carosign`; compile with selected codegen in `carosign-protocol`. Add semantic validators in `validation.rs`: exact ID lengths, phases, supported enums, packet/FD bounds, no security escalation from unknown fields. Never auto-add new protobuf fields. Acceptance P-02..P-06 on raw packet fixtures; external C implementation should parse same `.proto`.

## W05 — provider SDK reference worker

Files `carosign-provider-sdk/src/*`; implement private channel Hello/Init/Bind, mapped typed commands/events, cancellation channel separate from blocking operation, no real hardware. Provide **one simulated** Rust worker and a C-compatible protocol test fixture. Acceptance P-01..P-09; worker never self-authorizes new hardware resources.

## W06 — runtime actors / scheduler

Files `carosign-runtime/src/{operation/*,flow/*,scheduler/*,effect_runner,dispatcher}`. Wire pure Engine and ports, deterministic effects, per-operation serial actor and bounded events. Accepted factor only after verifier + authority revalidate. Acceptance E/A test matrix and P-10; no global lock across await.

## W07 — provider manager / discovery

Files `carosign-runtime/src/provider/*`, `carosign-linux/src/inventory`, `carosign-linux/src/process/*`. Load trusted package/profile manifest, matching with explicit ambiguity, verified spawn, pidfd monitor, epochs. Zero installed providers = healthy empty set. Acceptance P-12 and security checks.

## W08 — Linux IPC and service

Files `carosign-linux-ipc/src/*`, `crates/linux/carosignd/src/*`. Implement seqpacket protobuf frames, `SO_PEERCRED`, FD handling, `CallerContext` trusted construction, version Hello, event replay/gap, ACL. Start gate recovery before ready. Acceptance A-01..A-08.

## W09 — resource broker

Files `crates/linux/carosign-broker/src/*`, `carosign-linux/src/resources/*`. Policy-approved device claims, exclusive leases, allowed operations, FD handoff and quarantine. Use dummy device first. Acceptance FD mismatch/claim conflict, revocation with inherited FD, worker death. No unrestricted QSEE/RPMB RPC.

## W10 — evidence/authority

Files `carosign-evidence/src/*` and Runtime authority adapter. Implement software assertion evaluation (explicitly software) and protected verifier port with Unknown fail-closed. Do **not** implement fake hardware attestation. Acceptance A-07, E-01/E-06. Keep proof bytes out of public IPC.

## W11 — product shell

`carosignctl`, PAM, fprintd outgoing adapter stubs/real mapping as supported. CLI safe listing and op observation; PAM codes exactly [07](07-public-client-contract.md), no auto-install into PAM stacks. Acceptance that password fallback remains available, no direct hardware access by PAM.

## W12 — GXFP provider (separate repo)

Refactor known functioning capture/matching/template code into provider modules [14](14-provider-integration-contracts.md), conform to Provider wire; verify actual hardware workflow/format. No whole old `gxfp-authd` copy. Provide device-specific measurement of thresholds, not assumptions. Label actual hardware tests separately.

## W13 — C/GLib libfprint bridge (separate repo)

Use public libfprint with GCancellable and one GLib main context; no Rust ABI. Verify wire conformance with same fixtures. Prevent conflict with fprintd ownership, preserve FpPrint compatibility. Do not modify libfprint Core.

## W14 — Goodix QSEE provider (separate repo)

Implement from verified research command sequences; infrastructure QSEE/listener and IRQ separate. Native end-to-end enroll/verify only when actual device results proved. No assumed reconciliation command, no unreviewed RPMB mutation. Block on hardware blockers.

## W15 — Veles/Nord4 integration (external Veles repo)

Use Uzel/Поводник/Явь/veles-auth ports. Ensure no crate dependency from independent core to Veles. Hardware proof path and Trusted UI claims only after real end-to-end verification. No claim of full security based only on synthetic tests.

## Definition of Done per Wxx

- New public symbol precisely matches intended contract, error cases documented.
- Positive, negative, cancellation, stale-generation and crash/restart tests implemented for its scope.
- `cargo fmt`, `cargo clippy`, `cargo test` or equivalent commands reported with actual run status; no fabricated pass.
- Critical settings and security decision changes require ADR; no placeholders/TODOs on authorization, mutation or evidence success paths.
- Every filesystem mutation takes place in owned crate; no cross-layer SQL or Goodix command numbers.

### Mandatory v0.6.4 implementation amendment (W01–W14)

Every work order MUST treat matching, capture, image processing, score, templates and device-specific enrollment orchestration as provider implementation details. `W03`: `template_blobs` is an optional vault; do not create it as required state for all enrollments. `W04`: implement updated `provider.proto` 1.2 and strict binding validation. `W05`: write **three** simulated workers with equal wire conformance (host, MoC without templates, TEE with token) before declaring Provider SDK done. `W06`: Runtime routes pre-resolved authorized enrollment bindings; it performs **no** matching. `W10`: support ProviderAssertion with no token but reject it under hardware-only policies. `W12/W13/W14`: provider owns all matching. Scoped vault service is a separate ADR/STOP before implementing a provider relying on it; do not invent its IPC or staging. Conformance M-01…M-12 is mandatory. Normative details: [29](29-v064-provider-owned-matching.md).

### Mandatory v0.6.5 work-order additions (do not substitute proof)

Before W04 provider wire: add `common.enrollment_authorization`, provider 1.2 challenge frames, HAT ref/digest, scoped removal binding, strict parser tests H-01…H-10/S-01. Before W06 runtime enrollment: implement trusted challenge and credential/sink ports and correct prearm/armed phase transitions; prohibit raw token IPC. Before any Goodix/QSEE worker uses >64KiB hardware command: implement broker 1.1 sealed-memfd boundary, handler codec, and safe kernel/DMA ownership L-01…L-14; otherwise mark provider unsupported. Before W03/W07 mutation journal: implement storage schema v2 logical MutationPlan, migration and recovery R-01…R-10; never infer device physical state from Registry or restart. **Hardware-specific unresolved gates remain STOP conditions**, not implementation freedom. See 30–33.
