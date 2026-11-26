# 07. Operation coordinators and event delivery

## Modules (`carosign-runtime/src/operations`)

```text
operations/
  mod.rs
  common.rs                 # context, owner, deadline, epoch
  verify.rs                 # verify coordinator
  enroll.rs                 # enrollment workflow
  remove.rs                 # removal workflow
  identify.rs               # optional feature-gated operation
  cancel.rs                 # receipt vs completed cancellation
  observations.rs           # provider event mapping and filtering
  completion.rs             # terminal outcome assembly
  journal_bridge.rs         # durable mutation barriers
  evidence_bridge.rs        # verification and restricted result routing
```

## Verify pipeline

`Authorize → OwnerBind → ResourceAcquire → ContextCreate → ProviderVerify → MatchObserved → CurrentContextCheck → EvidenceEvaluation → ExternalResultDelivery → Release`. On retry/error, never reuse previous evidence. `Matched` means provider assertion; external acceptance is a separately verified result. Raw biometric material never enters normal event stream.

## Enroll pipeline

`AuthorizeManageFactor → QuiesceConflicts → DurablePrepare → AcquireLease → RevalidateGrant → MutatingDispatch → ProviderProgress → ProviderOutcome → IndependentReconcileIfRequired → RegistryGenerationCommit → DurableTerminal → NotifyOwner`. Once `MutatingDispatch` crossed, cancellation, timeout or worker crash may yield `NeedsReconciliation`.

## Remove pipeline

Same persistent semantics as enroll, but deletion may be irreversible; device-dependent absence query is not automatically proof of secure wipe. `RemoveRequest` must target current enrollment generation to avoid deleting replacement enrollment.

## Owner vs observer

`OwnerHandle` is revocable lifecycle/authority context. `ObserverHandle` can replay bounded progress under explicit permission and does not automatically cancel the operation. `OperationId` is lookup ID, **not** bearer authorization. A single operation may have multiple authorized observers but only one owner authority binding.

## Events

`OperationEvent {operation_id, sequence, kind}` sequenced per operation. Progress coalescible under pressure, terminal result durable for mutations and subject to retention policy for verification. Reconnect `after_sequence` may yield gap marker + snapshot, not fabricated history. UI prompts are semantic (`PlaceFinger`, `RetryRequired`), not provider-specific algorithm internals.

## Cancellation

`RequestCancel` returns immediate receipt (`Requested`, `AlreadyTerminal`, `Denied`), not `Cancelled`. A separate worker control pathway dispatches cancellation even while device thread is blocked; supervisor may quarantine instead of declaring release. Finalizers release interaction lighting and leases in all paths; failed cleanup changes device health.

## Error mapping

`NoMatch` is a valid check outcome; `Denied` is authorization failure; `DeviceUnavailable` is resource failure; `Indeterminate` applies to potentially persistent mutations. Strictly do not fold these into `Error(String)`.
