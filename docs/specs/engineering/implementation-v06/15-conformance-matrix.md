# 15. Conformance and adversarial acceptance cases

Conformance is **black-box over wire**; any language implementation must pass the same provider suite. Pure Core/Engine tests complement it. Hardware qualification is separately labeled: simulated success never proves secure hardware chain.

## Core / Engine tests (required)

| Test ID | Input | Required observation |
|---|---|---|
| C-01 | 16-byte zero ID | rejected at constructor |
| C-02 | same SubjectId with two distinct AuthorityIds | `SubjectRef !=`, distinct HashMap entries, distinct 48-byte scope prefixes and distinct 64-byte `enrollment_index` keys |
| C-03 | enrollment generation overflow | error; no mutation |
| E-01 | verify matched event then grant revoke | no accepted factor delivered |
| E-02 | stale worker_instance/device_epoch | rejected; state unchanged |
| E-03 | duplicate provider terminal | protocol fault; no second user result |
| E-04 | cancel before mutation arming | NoChange with confirmed no dispatch |
| E-05 | cancel after arming | NeedsReconciliation unless trusted no-change proof |
| E-06 | missing evidence/expired challenge | never accepted |
| E-07 | deterministic replay of same events | identical decision/effect list |

## Persistence / recovery

| Test ID | Fault injection | Required durable state |
|---|---|---|
| D-01 | process kill before `Prepared` commit | no mutation dispatched |
| D-02 | kill after `Prepared`, before `DispatchArmed` | safe no-change recovery |
| D-03 | kill just after `DispatchArmed`, before send | conservative NeedsReconciliation, no auto send |
| D-04 | kill after mutating send, no ACK | NeedsReconciliation, no replay |
| D-05 | kill after provider success, before Registry commit | reconcile hardware/registry |
| D-06 | kill after Registry commit, before journal Completed | recover via durable binding + query; no duplicate enrollment |
| D-07 | malformed DB record | fail closed, preserve original store |
| D-08 | generation conflict on delete | reject; new enrollment untouched |
| D-09 | provider unavailable after uninstall | registry survives |
| D-10 | incompatible template schema | refuse activation, preserve state |

## Provider wire transport

| Test ID | Injection | Required result |
|---|---|---|
| P-01 | supported Hello/Init/Bind | exactly one Ready after policy-approved lease |
| P-02 | incompatible major/minor | protocol error, no device access |
| P-03 | wrong 16-byte IDs/zero IDs | reject before side effects |
| P-04 | duplicate/out-of-order frame_seq | close worker session |
| P-05 | unsupported command/capability | typed Unsupported, no hardware mutation |
| P-06 | extra/missing ancillary FD | close FDs and reject |
| P-07 | invalid worker epoch on outcome | discard, no Registry transition |
| P-08 | cancel acknowledged while blocking operation | cancel receipt only; no fake quiesce |
| P-09 | worker crash after `DispatchArmed` | mutation pending and resource uncertain |
| P-10 | multiple workers claim same exclusive sensor | one lease or explicit denied |
| P-11 | C/GLib reference worker | same wire traces as Rust SDK |
| P-12 | zero installed providers | service healthy, zero authenticators |

## Public/authority

| Test ID | Scenario | Required result |
|---|---|---|
| A-01 | user supplies other's subject | authority denies/rebinds, never trusts field |
| A-02 | user with own UID calls Enroll w/o manage grant | unauthorized |
| A-03 | observer disconnects | operation remains, no implicit cancel |
| A-04 | owner disconnects during verify | suppress success, request cancel |
| A-05 | same idempotency key/different params | Conflict |
| A-06 | identical key/same params | existing operation, not second hardware call |
| A-07 | unsupported proof as KeyMint evidence | rejected; no fallback to software trust |
| A-08 | progress overflow | Gap+snapshot, terminal preserved |

## Installation / Linux

Package installs independently and provider manager sees it without recompilation; verified worker spawned from trusted path; worker cannot open unrelated device or network by default; PAM fallback documented and no installer modifies `/etc/pam.d`; fprintd and native libfprint worker cannot simultaneously claim same scanner. Testing root recovery path before PAM deployment is mandatory.

## Test fixtures

Wire fixtures must store exact serialized bytes (hex), expected validation errors and annotated state traces. No hard-coded chip IDs invented in generic conformance. Provider-specific hardware smoke tests are separate and record kernel/TA/profile versions, boot security state and actual observed errors. If physical validation not executed, label **not tested**, not pass.

## Additional v0.6.1 acceptance tests

| ID | Input/fault | Required result |
|---|---|---|
| D-11 | `template_blobs` key contains EnrollmentId+generation A, value mirrors B | `CorruptRecord` on read and migration; no decryption/matching, no implicit rewrite |
| D-12 | key/value agree but Registry subject/authenticator binding differs from AEAD AAD | `TemplateUnreadable` / fail closed; never NoMatch or fallback |
| P-13 | `FD_ROLE_MEDIATED_SERVICE` attached to `BindAuthenticator` with valid broker-issued resource | exact slot/resource validation; broker service channel only, and broker rejects unapproved method/payload |
| P-14 | mediated FD sent on `StartVerification`, or unknown/repeated `resource_id` | close all received FDs; reject packet before device side effect |
| P-15 | `FdSlot` has valid role but resource_id absent from current lease or wrong device epoch | binding rejected; broker grants no access |
| G-01 | Goodix mutation armed then worker dies; no verified TA query | `NeedsReconciliation`; locked scope; affected verification unavailable, no replay |
| G-02 | Goodix worker reports ready after restart | no recovery by itself; affected scope remains locked |
| G-03 | administrator clicks manual acknowledge without independent hardware state evidence | does **not** set Committed/NoChange or unlock affected scope |
| C-02b | two SubjectRefs share SubjectId but not AuthorityId, insert & lookup both | independent records and scope locks; equal/hash only when both fields same |

| P-16 | mutating mediated service call lacks trusted durable DispatchArmed binding | broker denies call, no hardware invocation, no fake completion |

## v0.6.2 B-series implementation-blocker resolutions

| ID | Setup | Required result |
|---|---|---|
| B-01a | Enroll recovery independently verifies expected new registration | `Reconciling → Committing → Finalizing → Terminal`; Registry CAS + journal atomic; terminal `Committed` |
| B-01b | Enroll recovery independently verifies *complete* unchanged pre-state | same phases; enrollment tables unchanged, journal/outbox persist NoChange, terminal `NoChange` |
| B-01c | Remove recovery independently proves target absent | desired-state commit removes Registry binding; terminal `Committed`, **not** `NoChange` |
| B-01d | Remove recovery independently proves exact original registration remains | no enrollment-table mutation; durable journal NoChange; terminal `NoChange` |
| B-01e | Enroll replace: new target absent but old target unexpectedly missing | `Indeterminate`, scope locked, **not** NoChange |
| B-01f | after verified reconciliation but before metadata durable | never `Terminal`; crash re-enters reconciliation or resumes proven durable metadata |
| B-02a | after `DispatchArmed`, Registry commit fails before write | `Reconciling`, affected scope blocked, no device resend |
| B-02b | Registry commit persists but ACK lost (`MetadataFailed`) | reread durable DB, reconcile without duplicate CAS or hardware command |
| B-02c | DB unavailable while persisting `NeedsReconciliation` | `StorageUnavailable`, safe blocking, no claim of durable uncertain record |
| B-02d | Verify has confirmed no-dispatch send failure | internal `Failed(DispatchNotSent)`, cleanup, public `Unavailable` for transport outage |
| B-02e | Verify send may have reached worker, ACK lost | internal `DispatchUnknown`, suppress all late success, block resource until quiesce |
| B-02f | recovery of durable `MUTATION_COMPLETED` | returns exact stored Committed/NoChange, never derives outcome from phase alone |

## v0.6.3 operation discriminant and dispatch compatibility

| Test | Scenario | Required result |
|---|---|---|
| E-08 | Attempt to construct Verification with mutation state | Cannot construct via public checked API |
| E-09 | Attempt to construct Enrollment/Removal lacking mutation state/plan | Cannot construct via public checked API |
| E-10 | Verification send receipt delivered to mutation body, and reverse | `InvalidInputForOperation`, zero effects, unchanged state/index |
| E-11 | Enroll(replace): old missing, new missing after attempted mutation | `Indeterminate`, scope remains locked |
| E-12 | Enroll(replace): verified entire unchanged original | `PreStateVerified`, Committing metadata NoChange |
| E-13 | Enroll(replace): expected new state fully verified | `DesiredStateVerified`, Committing Registry CAS |
| E-14 | Armed mutation local `MutationDispatchNotSent` or `MutationDispatchUnknown` | reconciling, no automatic retry/no false NoChange |

A separate persisted-plan format is not introduced by these tests: recovery requiring unknown pre/desired information is an explicit architectural blocker. See [28](28-v063-operation-discriminant-and-send.md).

## v0.6.4 mandatory strategy-neutral cases

The twelve mandatory cases **M-01…M-12** and expected results are normative in [29](29-v064-provider-owned-matching.md). Build three simulated provider strategies and run them through the same v1.2 wire fixtures. Not passing a hardware token is a valid *provider assertion*, never automatic hardware assurance. Actual hardware security must be tested separately.

## v0.6.5 mandatory architecture corrections and test IDs

**H-01…H-10** challenged enrollment, **L-01…L-14** broker large-body/DMA memory, **R-01…R-10** durable logical plan/recovery are normative in [31](31-v065-enrollment-authorization.md), [32](32-v065-bulk-command-buffer.md), and [33](33-v065-durable-mutation-plan.md). Every provider implementation must enforce them before claiming the corresponding capability. The provider test matrix now uses protocol **v1.2**, not historical v1.1; broker external bodies require minor **1**. Simulated traces do not certify Goodix hardware or credential trust.

**S-01** Provider 1.2 `StartRemoval` must carry `enrollment_binding` matching exact authorized EnrollmentRef/generation. For `SCOPED_VAULT`, `provider_opaque_ref` must be empty; for `PROVIDER_OPAQUE`, any mirrored value must exactly match the binding. Unscoped or mixed-binding deletion fails before hardware.

**S-02** A `QueryMutationResult` presence/absence from the same untrusted provider alone is insufficient for `DesiredStateVerified`/`PreStateVerified`. Independent trusted physical/storage verification is required, or outcome stays Indeterminate. **S-03** No hidden HAT, raw payload or biometric matching implementation in Engine/Core/Runtime.

## v0.6.6 checked create/replace scenarios

The required B14-01..B14-12 scenarios are specified in [34](34-v066-enrollment-command-projection.md). In particular, replace missing old binding, corrupt scope snapshot, wrong echoed planned identity, incompatible minor and partial physical change all fail closed. These are normative acceptance cases, NOT tests already executed.
