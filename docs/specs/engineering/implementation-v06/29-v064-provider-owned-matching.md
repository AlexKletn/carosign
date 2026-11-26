# 29. v0.6.4 — Matching is exclusively provider-owned (M-01…M-12)

**Normative decision (11.11.2026).** This section supersedes host-matching assumptions anywhere in v0.6.x, including the template-storage baseline in document 10. No implementation agent may invent alternate generic matching, capture, scoring, or template algorithms. All device-specific algorithms and TA procedures belong to an independently installed provider. The external provider can be written in any language; its contract is `carosign.provider.v1`.

## D-01 — Single operation contract, zero core matching

`carosign-core` and `carosign-engine` MUST NOT contain or depend on `Matcher`, `CaptureSession`, fingerprint image, feature extractor, score, FAR threshold, template format decoder, neural network model, chip register or TA command. They handle authorization, operation lifecycle, a *logical* `EnrollmentRef`, durable mutation state, scope locks, resource leases and external evidence/assessment. The Runtime dispatches `BeginVerification` to exactly one selected authenticated provider. It does not carry out matching and does not interpret provider-specific biometric data.

Internal operation phase is `Running`, not mandatory `Capturing -> Scoring -> Matching`. The worker may perform any number of captures, swipe passes, feature extraction or TA commands internally; it reports only optional semantic progress and exactly one outcome. Even `PROGRESS_CAPTURING` is an optional UX hint, never a required FSM input. A provider can return a result without ever producing a sample/progress event visible to Runtime.

## D-02 — Every provider model is a valid first-class implementation

1. **Host matcher** (GXFP): provider receives permitted enrollment references; captures/decodes frames, matches and owns any native template format. Its runtime/matcher is private to its package.
2. **Match-on-chip** (MoC): provider commands sensor-specific enrollment/verification; sensor may store all templates; no raw capture and no exported templates are required. Provider returns a scoped match assertion and optional token.
3. **TEE matching** (Goodix/Nord4): provider speaks device/TA protocol, owns listener/session coordination, holds only opaque TA references; raw templates, raw touch data and TEE internals are not exposed to Core.
4. **libfprint bridge**: C/GLib process wraps the installed libfprint version, whether the selected driver internally uses software matching or on-device matching. Core is unaware.

Neither `TemplateCustody` nor the existence of a hardware security environment determines where matching happens or the assurance of the outcome. Device/provider claims do not constitute independently verified assurance.

## D-03 — The Runtime routes explicit subject-scoped bindings

For v1 verification, a trusted authority first resolves the claimed `SubjectRef`. Registry then resolves the **current authorized set** of enrollments for `(subject, authenticator)`. Runtime rejects `BeginVerification` with `NotEnrolled`/typed unavailable when no active authorized bindings exist; no unscoped 'scan all chip slots' fallback. It sends only bounded logical `EnrollmentBinding` values to the worker. Each binding includes `EnrollmentRef`, storage kind, and either a provider-opaque reference or the identity permitting access through an explicitly scoped vault service. There are no image/template bytes on this control channel. The provider must restrict the actual match to this target scope, and MUST NOT treat raw `SubjectId` as a device-native user identifier without a verified provider-owned binding; otherwise fail `UnsupportedSubjectScoping` — never turn an unrestricted 1:N result into a match for an arbitrary claimed subject.

`EnrollmentBinding` is **not** an authority grant or permission to access another subject; Runtime / broker pre-authorize exact owner, authenticator, enrollment and generation. For provider-managed custody, the provider interprets the 0..4096 byte opaque reference (an empty reference is permitted only where provider-held persistent state reconstructs the exact scoped binding; otherwise fail); for vault-backed custody it uses the broker's restricted storage facility. The core does not parse either. Neither provider-ref nor label is guaranteed to uniquely identify a user across different hardware. Device identity and epochs still fence old bindings.

A successful verification may optionally identify which permitted enrollment matched. If `matched_enrollment` is supplied it MUST be a member of the authorized set with exact generation, or Runtime discards the result as a provider violation. If the hardware cannot return an enrollment ID, the provider must still prove to its own adapter logic that the match was confined to the authorized scoped set; where this cannot be achieved, return failure, not Matched.

## D-04 — Mutation operations remain generic; providers own side effects

`Enroll` creates a new platform `EnrollmentId`, not a template. Provider may enroll via software templates, chip slots or TA registration. `Enroll(replace)` passes immutable old `replace_binding` together with the expected prior `EnrollmentRef`; the provider may require an internal multi-step replace and reports only observed result and provider opaque reference. `Remove` passes a scoped binding, not raw template bytes. **A provider may change hardware state on any operation step declared mutating**; Runtime arms the durable mutation before dispatching the earliest potentially mutating call, not only before an imagined `SaveTemplate` step.

`ReconciliationVerifier` still compares the complete `pre/desired` states. For devices without safe independent query, remain `Indeterminate`, quarantine affected scope, and do not replay mutation. The persistent canonical `MutationPlan` is still an *open architecture blocker*; this delta does not invent its binary format.

## D-05 — Storage service is OPTIONAL, never a matching service

`template_blobs`, `ProtectedTemplateRecord`, `KeyProvider` and AES-GCM are a **platform-provided optional encrypted blob vault** for providers that need to store templates *off-sensor*. These records are not universal biometric registrations and do not impose any template format on hardware-backed providers. The Runtime owns Registry metadata and mutation journals; an approved vault/storage adapter enforces scoped access to encrypted blobs. Plaintext is only handled within appropriately authorized provider/storage boundaries, never by generic `Engine` or `EvidenceVerifier`.

`CUSTODY_PROVIDER_MANAGED`: provider/chip/TA manages templates or other native private storage, Registry keeps bounded opaque references; no `template_blobs` entry is required. `CUSTODY_PLATFORM_MANAGED`: optional platform vault stores opaque encrypted provider bytes, but **provider** still performs capture, comparison and interpretation. Missing requested vault service prevents activating a provider that requires it; it does not cause core to create templates or match them.

An enrollment can have zero provider-exported template bytes and still be valid. Deleting the provider package never implicitly deletes device-managed templates. Format/version and migration are provider responsibilities; Runtime only checks metadata bindings and consistency.

## D-06 — Matched does not require a hardware token

A worker's `VERIFY_MATCHED` is an **unverified provider-origin assertion**. It does not need to carry an `opaque_token` or any software score. If the wire frame has no explicit `evidence`, the Runtime creates an internal **ProviderAssertion** evidence handle scoped to `(operation, worker instance, device epoch, authorized subject/enrollments, purpose, recipient, challenge)` based on authenticated IPC/correlation. This handle is a local assertion, **not** a cryptographic hardware credential. Its acceptance is subject to platform authentication policy and assessed worker trust. A missing hardware token MUST NOT be silently fabricated.

If the provider supplies a token, an independent compatible verifier must evaluate it; if absent/invalid it cannot satisfy policies requiring a hardware-backed credential. Core `VerificationOutcome::Matched(EvidenceId)` is retained because Runtime always creates an evidence record (either assertion or opaque material reference) after validation; the worker never directly chooses an `EvidenceId` in the trusted namespace. For a successful match without provider token, evidence mode is `ProviderAssertion`. Scoring thresholds/quality remain private to provider even if software-based.

## D-07 — Wire amendments to `carosign.provider.v1`

`StartVerification` MUST include a non-empty, bounded `permitted_enrollments` list of `EnrollmentBinding`; the exact canonical subject and authenticator already occur in context. `StartEnrollment` adds optional `replace_binding` and MUST check consistency with its existing `replace` field; either both present and same ref, or both absent. `StartRemoval`'s enrollment and opaque ref are checked against Registry. `VerificationOutcome` can optionally include `matched_enrollment`, which must belong to the permitted set.

`EnrollmentBinding.kind` distinguishes `PROVIDER_OPAQUE` from `SCOPED_VAULT`. `SCOPED_VAULT` means **lookup by EnrollmentRef through an already authorized mediated vault**, not a free pass to open the metadata database. The vault protocol and durable provider-write staging are a separate architecture gate before the vault-backed provider is shipped; no execution agent may invent it. For v1 workers that use provider-managed sensor/TA storage, `PROVIDER_OPAQUE` path suffices and does not require the vault.

Provider wire protocol minor bumps to **1** for these new messages; protocol `major=1`, `minor=0` remains structurally decodable but lacks target-scoped verification for this architecture and MUST NOT negotiate for real biometric operations. This is a draft/pre-freeze incompatibility, not a promise of working interop with previously generated v1.0 workers. `WorkerHello` and `Initialize` must negotiate minor >=1; providers implementing only minor 0 fail activation in this architecture. Feature support does not substitute for this mandatory semantic change.

## D-08 — Protected interfaces / single writer

No service call `Match`, `CompareTemplate`, `ExtractFeatures` or `CaptureRawImage` is part of central client/provider/evidence protocols. A separate opt-in diagnostic interface may expose raw sensor diagnostics under special permission and privacy controls, but must never influence auth policy by returning user-supplied scores. Platform-bound grants remain scoped; the provider cannot select subject or elevate result assurance.

## Conformance cases (all mandatory for the matching boundary)

- **M-01:** Simulated host worker does frame processing internally; Core has no sample/template/score methods.
- **M-02:** Simulated MoC worker verifies against opaque slot reference with no exported image/template; normal success works.
- **M-03:** Simulated TEE worker supplies opaque token; Core only routes it, verifier accepts/rejects independently.
- **M-04:** MoC worker emits Matched without token; runtime creates ProviderAssertion; hardware-required policy rejects it.
- **M-05:** Worker reports `matched_enrollment` outside request set; Runtime rejects outcome and releases/quarantines as required.
- **M-06:** Empty/foreign/stale enrollment set; Runtime never asks worker to scan all users.
- **M-07:** Worker returns successful outcome with no capture/progress event; engine accepts flow.
- **M-08:** Provider-managed TA enrollment succeeds with no `template_blobs` write or software matcher.
- **M-09:** Provider declares TEE/MatchOnChip but no hardware verifier exists; no hardware assurance is granted.
- **M-10:** `Enroll(replace)` sends old binding + new mutation id; partial result remains reconciliation, not NoChange.
- **M-11:** TA cannot independently query changed state; pending mutation remains blocked and cannot auto-retry.
- **M-12:** Provider requests optional vault but no audited scoped vault service is available; fail activation without falling back to software matching in Runtime.

## Explicit STOP conditions for implementation agents

- Do not implement `carosign-core::Matcher`, shared sample format, NCC/threshold fields, template decoder, hardware algorithm policy or image pipeline.
- Do not assume all hardware exports a fingerprint template or matched enrollment ID.
- Do not classify MoC or TEE as verified hardware merely from capability bits.
- Do not invent vault IPC or a binary mutation plan without separate ADR.
- Do not permit unscoped match on a claimed subject, even for a legacy/libfprint adapter.
- Do not promote a worker-produced `Matched` into auth success without external policy/evidence evaluation.

## Canonical wire traces — no mandatory host matching

All strategies use the same lifecycle protocol: `WorkerHello -> Initialize -> BindAuthenticator -> AuthenticatorReady`, then `StartVerification(allowed bindings) -> CommandAck -> [0..N ProviderProgress] -> VerificationOutcome`.

- **Host**: worker privately retrieves its scoped template material, captures and compares; `VERIFY_MATCHED` may carry no hardware token; trusted Runtime records provider assertion.
- **Match-on-chip**: worker privately resolves allowed slot IDs; chip matches internally; same `VERIFY_MATCHED` may have no image, template or token. If chip cannot restrict to allowed slots, worker reports unsupported.
- **TEE**: worker maps allowed enrollment binding to scoped TA identity, runs TA verification, returns `VERIFY_MATCHED` with optional opaque hardware token; only independent token acceptance can satisfy hardware-only policy.

The common path **never** includes `Capture -> ExtractFeatures -> Score -> Compare` steps. It has no assumption that the provider implements `Enumerate`, `QueryMutationState`, template export or token generation.

For `StartEnrollment`, all three strategies receive the same authorized mutation reference. The worker may prompt for touches, silently enroll on-chip, or call TA; it returns `MutationOutcome` and bounded provider refs after hardware work. Runtime does not create biometric templates.

### v0.6.5 historical version note

This v0.6.4 decision remains normative for matching ownership. Its quoted `provider minor >=1` was the minimum for **v0.6.4**, not the current package. Current v0.6.5 requires provider **minor >=2** for challenged enrollment and scoped removal. See [30](30-v065-audit-and-authority.md), [31](31-v065-enrollment-authorization.md). No host matcher is introduced.

**v0.6.6 refinement:** Provider-owned matching is unchanged. For enrollment, trusted Runtime now explicitly constructs a typed Create/Replace `CommandPlan` and passes the validated old binding only for REPLACE. `StartEnrollment` additionally carries system-planned NEW enrollment ref (provider minor >=3); an opaque future provider ref is never guessed. [34](34-v066-enrollment-command-projection.md).
