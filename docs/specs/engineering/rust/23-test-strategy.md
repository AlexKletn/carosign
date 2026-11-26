# 23. Tests and conformance by module

This is an implementation acceptance plan, **not** claim tests have already been run for new platform.

## Test matrix

| Layer | Unit | Integration | Fault injection |
|---|---|---|---|
| core | IDs, validation, limits | serde mapping through adapters | malformed encodings |
| engine | transitions + invariants | event replay across crash | reordered/stale inputs |
| runtime | owner, observer, scheduler | simulated provider | hang, slow client, cancellation |
| persistence | records + CAS | crash/reopen and transaction barriers | cut power simulation, corrupt DB |
| provider manager | matcher/resolver/version | fake process supervisor | worker crash, bad manifest |
| IPC | framing/FD slots | cross-language C/Go/Rust fake workers | truncation, fuzz, unauthorized |
| evidence | context and acceptance | dummy verifier + Veles adapter | replay, wrong token/epoch |
| Linux broker | claims and FD ownership | Linux namespace test | revoke raw FD, reboot |
| providers | protocols, hardware state | device full loop | IRQ loss, TEE listener failure |

## Black-box provider conformance

Unmodified external worker launched with assigned private IPC channel. Required cases: Hello negotiation; wrong version; unknown opcode; command correlation; wrong epoch; cancellation receipt vs outcome; one outcome per op; malformed FD; crash after mutation dispatch; repeated idempotency key; package reinstall recovery. Language independent test driver owns all assertions.

## Persistence fault windows

1. before journal Prepare;
2. after `DurablePrepared`, before dispatch;
3. after `DispatchPossible`, before provider response;
4. after provider completion, before Registry commit;
5. after registry+journal transaction, before client notification;
6. crash during reconcile.

In each window, recover without blind mutating TA resend. If provider cannot independently confirm hardware state, show `NeedsReconciliation` and block conflicting scope.

## Acceptance on real hardware

GXFP: enroll → verify → remove → reboot → reinstall; quality/FAR/FRR evaluated separately. Goodix: direct native full cycle, persistence, listener poison/cancellation, alternative finger rejects. Nord4: trusted input/FOD lifecycle and hardware token→KeyMint if claimed; absent full proof cannot market strict assurance.

## Static checks

Dependency graph CI forbids platform deps in core/engine; `cargo deny`/license checks as appropriate; source format lint, generated `.proto` diff/compat; manifest path safety; `cargo test` for Rust crates and cross-language conformance for C bridge. Do not claim protocol freeze before full actual `.proto` generation.
