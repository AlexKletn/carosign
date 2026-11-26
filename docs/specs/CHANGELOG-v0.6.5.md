# Biometric Platform Architecture — v0.6.5 changes

**Release type:** architecture/specification correction only; not executable implementation or hardware-certified conformance.

## Normative changes

- B-10: add provider enrollment challenge + reference-only HAT authorization contract (provider protocol v1.2; capability flag; trusted broker/sink and credential issuer). Detailed constraints: engineering/implementation-v06/31-v065-enrollment-authorization.md.
- B-11…B-13: add typed broker bulk body and result via one bounded, sealed memfd on **broker protocol v1.1 only**; DMA32, buffer lifetime, cache coherency, fault/retry semantics. Detailed constraints: engineering/implementation-v06/32-v065-bulk-command-buffer.md.
- A-15: define persisted immutable `MutationPlanRecord` in `bio.storage.v1` with storage schema version 2, pre/desired predicates and explicit recovery limits. See engineering/implementation-v06/33-v065-durable-mutation-plan.md.
- Add required `StartRemoval.enrollment_binding` for scoped vault as well as provider-opaque enrollment refs.
- Synchronize provider v1.2 minor negotiation samples and normative dependency/actor/error/conformance references.
- Preserve provider-owned matching, image processing, feature extraction and template custody; do **not** add Core/Runtime matcher.
- Add independent finding/precedence matrix: engineering/implementation-v06/30-v065-audit-and-authority.md.
- Reconstruct and clearly label `27-enrollment-challenge-gap.md` because the original was absent from the supplied v0.6.4 ZIP.

## Explicitly open

Provider/hardware success and independent mutation verification have **not** been shown. In particular Goodix secure storage failure 1035 in archived experiments does not demonstrate successful enrollment. Safe rollback/reconciliation and actual broker module/TA UAPI need device-specific proof. Real HAT issuance/validation, hardware-compatible DMA, and audited broker handlers are integration prerequisites; lacking them means feature unavailable, not permission to bypass.

## Verification limitations

Static schema sanity, Markdown non-reference links, manifest file hashes and ZIP CRC are checked at packaging time. No protoc, independent Rust/C interoperability, Rust compilation or hardware tests in this environment. Historical research references contain 14 unresolved links pointing outside the imported ZIP.
