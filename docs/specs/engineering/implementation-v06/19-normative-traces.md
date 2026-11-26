# 19. End-to-end message and persistence traces

Only semantics are normative; `CommandId` values illustrate correlation. Every trace assumes validated identities, epochs and supported operation kinds.

## T1 — provider bootstrap

1. Supervisor verified spawn → private seqpacket; child sends `WorkerHello` (seq 1).
2. Manager selects major 1/minor 2, sends `Initialize` with instance ID/profile; worker `Initialized`.
3. Manager requests resource set broker; broker approves atomic lease and FDs.
4. Manager sends `BindAuthenticator` + role-tagged FDs; worker verifies profile/resource list and replies `AuthenticatorReady`.
5. Manager publishes authenticator only after lease/device health and capability assessment; provider ready alone does not claim trusted hardware.

**Failure:** wrong frame seq/FD count/epoch results in worker shutdown; acquired leases quiesced or marked uncertain. No published authenticator.

## T2 — software verification (GXFP)

1. Auth consumer `BeginVerification` → trusted subject/purpose/grant resolved.
2. Runtime queues and acquires resource lease; revalidates grant.
3. Provider `StartVerification` with scoped subject; sends `CommandAccepted`, semantic progress, then `Matched` with software assertion reference.
4. Runtime validates epoch/context, EvidenceVerifier evaluates software policy, revalidates owner, returns `AcceptedFactor` *for that request only*.
5. Cleanup releases resource physically; if owner revoked before delivery, suppress accepted result.

**Failure:** `Matched` received after cancel or from older worker → no success. No KeyMint hardware assurance.

## T3 — TEE verification with protected recipient

1. Protected consumer creates/accepts challenge in its domain; `BeginVerification` binds recipient/challenge to operation.
2. Provider invokes TA; obtains opaque token; raw bytes do not go through public client API.
3. Restricted EvidenceRouter transfers token to compatible independent verifier/recipient.
4. Only verifier acceptance for current challenge/recipient/purpose may produce `AcceptedFactor`; if hardware verifier absent, return unverified/unavailable, not software fallback for protected key operation.

## T4 — durable enrollment and crash

1. ManageGrant issued/revalidated; Runtime `MutationId=M1`, persists `Prepared`/scope lock.
2. Resource acquired and grant revalidated; Runtime persists `DispatchArmed` + outbox `CommandId=C1`.
3. Runtime sends `StartEnrollment(M1,C1)` once.
4. Worker crashes before response. Journal still `DispatchArmed`; supervisor quarantines device; Runtime publishes `NeedsReconciliation` (not Cancelled/NoChange).
5. New worker may execute only approved safe query. Verified actual state + matching identity/generation permits registry repair or confirmed no-change; unsupported query leaves M1 pending. No second `StartEnrollment` automatically.

## T5 — removal with generation mismatch

1. Client asks `Remove(enrollment_id=E, expected_generation=G)` with manage grant.
2. Registry already has `G+1`. Runtime returns `GENERATION_CONFLICT` before command dispatch; no device side effects.
3. Public response does not reveal other user's enrollment contents.

## T6 — worker replacement after upgrade

1. Stage/validate new package, stop admission, drain operations.
2. Confirm device Quiesced; migrate provider-owned state with journal if needed.
3. Old worker exits, new worker gets new `WorkerInstanceId`, bumped `DeviceEpoch`; stale messages from old worker are rejected.
4. Registry retains enrollments; only verified same physical authenticator and compatible template format are rebound.

## T7 — cancellation vs hardware callback

1. Owner withdraws grant while provider captures. Actor records `CancellationRequested`, closes evidence delivery path and sends cancel via independent control channel.
2. Provider ACKs request but blocked call may still run. `CancelAck` alone cannot free lease.
3. If hardware quiesces, release lease and finalize Cancelled. If mutation already armed or hardware cannot confirm stop, persist `NeedsReconciliation`/device `Uncertain`.

## T8 — confirmed reconciliation after armed mutation

1. `M1` is durable `DispatchArmed`; worker lost. Scope remains locked.
2. Trusted reconciler independently proves desired **post-state**, with matching identity and generation. Engine: `Reconciling → Committing`, applies Registry CAS+journal atomically, retains scope lock. After confirmed cleanup and durable finalization with `terminal_outcome=COMMITTED`, enters `Terminal` and unlocks.
3. Alternatively, reconciler independently proves full original **pre-state**. Engine still enters `Committing`; transaction leaves enrollments/index untouched but records `MUTATION_NO_CHANGE`+outbox. After cleanup/finalization stores `terminal_outcome=NO_CHANGE`, enters `Terminal`, unlocks.
4. For `Remove` the *post-state* is absence, so removal verified absent is Committed, not NoChange. Partial or mismatched observations remain NeedsReconciliation.

## T9 — lost DB ACK vs lost verification dispatch

1. Mutation reached `DispatchArmed`. Metadata commit reports `MetadataFailed`; Runtime quarantines scope and enters `Reconciling`. It never infers that Registry did not commit, never repeats provider mutation. After storage resumes, it reads durable Registry+journal and resolves via validated state/independent query, or remains blocked.
2. A **verification**, which has no `DispatchArmed`, fails before command send with validated `NotSent` receipt: internal `DispatchNotSent`, public transport outage mapped to `Unavailable`, resource cleaned up.
3. If command delivery cannot be ruled out, internal `DispatchUnknown`; suppress late success and quarantine until device quiesces. Public `NotMatched` is forbidden in both cases.

### v0.6.3: disjoint send receipts and partial replacement

1. Given `OperationBody::Enrollment { request.replace=Some(old), ... }`, a correlated `VerifyDispatchNotSent` input is rejected (`InvalidInputForOperation`) with no effects and no state/index change.
2. After durable arm, `MutationDispatchNotSent` is recorded but cannot by itself prove unchanged TA state. Engine enters/remains reconciliation with the scope blocked and does not resend.
3. Independent query shows old registration absent and new registration absent. It matches neither full pre nor desired post-state: `Indeterminate`, no `CommitNoChangeAndJournal` and no `CommitDesiredRegistryAndJournal`.
4. A complete old registration unchanged returns `PreStateVerified`; committed metadata is NoChange. A complete new registration with required old replacement returns `DesiredStateVerified`; commit performs Registry CAS. In both cases quiescence and durable finalization precede terminal delivery.

## T10 — HAT-required enrollment (provider 1.2, B-10)

1. Authorize managing subject, persist `Prepared` with complete v2 MutationPlan, acquire lease, determine `ENROLL_AUTH_HAT_CHALLENGE` and compatible trusted credential/sink.
2. Provider `PrepareEnrollmentChallenge` (audited non-persistent pre-enroll only) → `CommandAck` → `EnrollmentChallengeReady(challenge, challenge_ref)`. Trusted broker independently binds live hardware session; a worker-only challenge is insufficient.
3. Trusted credential authority performs step-up, creates restricted single-use `EnrollmentAuthorizationRef` bound to mutation, scope, current worker/epoch, challenge and TA recipient. No raw HAT crosses provider or client IPC.
4. After grant revalidation and durable `DispatchArmed`/outbox, Runtime sends `StartEnrollment(MutationId, auth_ref, SHA256(challenge))` once. Audited broker handler validates armed mutation, consumes HAT and injects token only into approved TA command. Provider handles capture and device-specific post-enroll.
5. Provider success is not Registry commit; CAS and safe cleanup must succeed. Losing HAT/worker/broker after arm -> reconciliation. Without actual hardware-safe query it remains blocked.

## T11 — large typed hardware body through broker (B-11..13)

1. Trusted profile admits fixed typed hardware handler; provider prepares <=8MiB body in immutable sealed memfd, broker call carries `external_body` metadata and one FD on **broker** socket (not provider-control).
2. Broker validates FD, identity, bounds, digest, hardware typed schema, lease, effect class and durable arm for mutating handler.
3. Trusted kernel/QSEE adapter allocates correctly sized bounded DMA, copies and syncs bytes, checks address-width/length and only then invokes audited hardware command; retains memory until verified hardware completion.
4. Reply returns inline typed status or restricted sealed response memfd; errors/unknown completion do not free in-flight DMA or replay an armed command. Cleanup/quiescence follow 32.

## v0.6.6 traced enrollment dispatch projection

**Create:** validated EnrollRequest.replace=None -> durable v3 plan ENROLL_NEW with assigned new ref B -> Runtime `Create(planned_new=B)` -> durable arm -> provider v1.3 `StartEnrollment(planned_enrollment=B, no replace fields)` -> response `MutationOutcome.enrollment=B` -> independent postcondition validation -> metadata CAS.

**Replace:** validated replace=Some(A) -> durable v3 plan ENROLL_REPLACE, `old_target=A`, pre-snapshot containing A + other affected registrations, assigned new B (B != A) -> under scope lock Runtime verifies A against exact Registry (including binding kind/digest) -> `Replace(planned_new=B, old=A_binding)` -> durable arm -> provider `StartEnrollment(planned_enrollment=B, replace=A, replace_binding=A_binding)` -> independent three-way verification. Observed A absent and B absent yields Indeterminate; no `NoChange` and no replay.

These override old minor 2 examples. [34](34-v066-enrollment-command-projection.md).
