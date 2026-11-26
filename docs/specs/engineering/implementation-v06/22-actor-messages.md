# 22. Runtime actors: concrete command/observation contracts

## One-writer principle

One `OperationActor` owns one `OperationState` and calls pure `engine::decide`. All messages from client, owner watcher, provider, broker, evidence verifier or DB enter its bounded mailbox. No other service writes this state. Each message is validated against operation ID, epoch and phase before transition. Events from an older `WorkerInstanceId` cannot modify current operation.

## Operation mailbox discriminant

```rust
pub enum OperationMsg {
    Authorized(ValidatedGrant),
    GrantRevoked(GrantRevocation),
    LeaseGranted(ResourceLease),
    LeaseFailed(ResourceError),
    IntentPersisted(DurableMutationReceipt),
    DispatchArmed(ArmedMutationTicket),
    ProviderAccepted(CommandId),
    ProviderProgress(ProviderProgressObservation),
    ProviderTerminal(ProviderTerminalObservation),
    EvidenceEvaluated(Result<EvidenceAssessment, EvidenceFailure>),
    MetadataCommitted(RegistryCommit),
    MetadataFailed(StorageFailure),
    ReconciliationResult(ReconciliationVerdict),
    VerifyDispatchNotSent(DispatchFailure),
    VerifyDispatchUnknown(DispatchFailure),
    CleanupConfirmed(QuiesceAssessment),
    OwnerDetached,
    DeadlineElapsed,
    CancelRequested(CancelSource),
    WorkerLost(WorkerInstanceId),
    DeviceEpochChanged(DeviceEpoch),
}
```

Types in this list are **trusted, internal, validated domain observations**. `ProviderAccepted` carries only command accepted, not hardware effect. `ProviderTerminal` may lead to evidence checking or mutation reconciliation; never direct login success. `DeadlineElapsed` from injected clock watcher, not arbitrary client timestamp.

## Mailbox priority and ordering

Use two bounded channels per actor: control (capacity 16) and normal (capacity 64). Control includes `GrantRevoked`, `OwnerDetached`, `CancelRequested`, `WorkerLost`, `DeviceEpochChanged`, `DeadlineElapsed`; normal includes progress, receipts and completions. Consume control **before** normal when both ready. Within each channel FIFO. If control queue capacity exceeded, supervisor closes operation to safe error and persists any armed mutation uncertainty; do not drop a revocation signal and continue success.

Avoid starvation: after 8 control messages, process one normal message *only if* no pending revocation invalidates it. Each normal message still receives fresh grant/epoch validation before state transition. This scheduling policy is **internal implementation detail** but its security invariants are fixed.

## Provider-session actor

One `ProviderSession` owns IPC connection+worker instance and maps external `ProviderFrame` to validated `ProviderObservation`. Uses a single reader task, single writer task and independent cancellation/control submission. Blocking hardware happens **inside worker**, not Runtime. Session checks sequence and command IDs, discards wrong instance/epoch, reports worker loss to all active op actors, and never edits Registry directly.

## Scheduler actor

`Acquire(ResourceSet)` is atomic against the graph. Maintain a per-resource owner/fence, queued priority classes `SystemAuth > InteractiveManage > Diagnostic`, FIFO within priority. Anti-starvation: an interactive request waiting longer than 30 seconds gains next available slot **but never preempts an active mutation**. This policy is a proposal locked for baseline; changing priority or starvation behavior requires ADR because it impacts authorization latency and resource exposure. Queue length bounded; no indefinite reservations.

## EventHub

Progress events coalescible, monotonic sequence per operation. EventHub emits `Gap+Snapshot` on replay discontinuity. Terminal mutation result is **derived from persistent state** and visible under observer authorization; no dependence on in-memory ring retention. Owner reconnect requires fresh policy authorization, not possession of `OperationId`.

## Cleanup supervisor

Runs independently of caller connection. Holds ownership of `ResourceLease`, `InteractionLease` and WorkerSession until they are safely quiesced or explicitly quarantined. On panic, panic boundary reports worker fault; does **not** claim hardware quiescence. On shutdown, unfinished mutation persists for recovery. No resource freed by only dropping local struct in an unknown hardware state.

### Validated engine observations for B-01/B-02

The generic `MetadataCommitted(RegistryCommit)` is normalized to `MetadataDurable(Committed|NoChange)` before Engine input. The `OperationMsg` enum above contains the added typed `MetadataFailed(StorageFailure)`, `ReconciliationResult(ReconciliationVerdict)`, `VerifyDispatchNotSent(DispatchFailure)`, and `VerifyDispatchUnknown(DispatchFailure)` observations. These observations must be correlated to a pending effect/operation, never accepted from a provider wire field directly. Reconciliation and storage failure are control-priority events; no success may overtake them. Verification `DispatchUnknown` suppresses late provider outcomes and requests quiescence. No `SendFailedBeforeArm` input exists for Verify. See [27](27-v062-engine-conflict-resolutions.md).

### v0.6.3: machine-specific dispatch acknowledgements

The outer actor mailbox remains a transport-agnostic dispatcher, but `ProviderAccepted` alone does not decide machine semantics. The effect runner must correlate the ack or send fault to the pending effect's kind and normalize it to `VerifyDispatchAccepted/NotSent/Unknown` **or** `MutationDispatchAccepted/NotSent/Unknown` before passing it to Engine. `SendVerify` and `SendMutatingCommand` receipts are disjoint. On mismatched `OperationBody`, reject with `InvalidInputForOperation`, no transition/effect/index change. An armed mutation remains reconciliatory even when the local send reports `MutationDispatchNotSent`. See [28](28-v063-operation-discriminant-and-send.md).

### v0.6.5: enrollment preparation receipts and priority

Add internal typed `ChallengePrepared(ValidatedChallenge)`, `ChallengeUnavailable(ChallengeFailure)`, `EnrollmentCredentialAuthorized(EnrollmentAuthorizationRef)`, and `EnrollmentCredentialDenied(CredentialFailure)` observations, valid only for `OperationBody::Enrollment` with `ENROLL_AUTH_HAT_CHALLENGE`. These observations are emitted by trusted adapters after scope/correlation checks, never constructed from unchecked worker bytes. Their machine inputs are disjoint from `SendMutatingCommand` receipts. Revalidation and token expiry are control-priority checks before arming; `GrantRevoked`/`CancelRequested` invalidates pending token refs. Once armed, a missing/expired auth ref is reconciliation, not a new attempt. Buffers and hardware completion are broker-owned and **never** stored in OperationState.

### v0.6.6 — typed new/replace and no inferred command

The mutation effect runner must derive the exact typed `CommandPlan::Enrollment(Create|Replace)` using `EnrollmentCommandBuilder` from the v3 immutable prepared plan and checked Registry snapshot; for a replacement it carries `old: ValidatedEnrollmentBinding`. Generic `MutationChange::Enrollment { old: None, desired: None }` is prohibited. Runtime constructs this projection before durable arm; after arm, loss or invalidation is a reconciliation fault, never an invitation to issue a fresh command. Cross-operation/epoch mismatch is rejected. [34](34-v066-enrollment-command-projection.md).

## Привязка и одноразовая отправка изменения

<a id="mutation-dispatch-binding"></a>

### Привязка изменяющей отправки к сохранённому актору

`BindMutationDispatch` допустим однократно для enrollment/removal в `Arming` с единственным точно адресованным ожидающим `PersistDispatchArmed`, согласованными выдачей и эпохой и при отсутствии отмены. Владелец проверяет полномочие, срок, живую выдачу, её полного владельца и режимы. Движок закрепляет worker, lease и команду; адрес команды определяется следующим переходом с `DispatchArmedDurable`. Привязка сама не выполняет arm или отправку.

#### Точный договор

1. Добавить в `DomainInput` отдельный `BindMutationDispatch` с адресом
   ожидающего `PersistDispatchArmed`, `LeaseId`, `WorkerRef` и `CommandId`.
   Это внутренний вход Engine, он не меняет protobuf или формат хранения.
2. Для `Enrollment` и `Removal` разрешить его только в `Arming`, до
   постановки подтверждения arm, при единственном ожидающем эффекте
   `PersistDispatchArmed`, без отмены и без ранее закреплённого работника или
   команды. Если лизинг уже закреплён, он должен совпадать. Эпоха работника
   обязана совпадать с эпохой аутентификатора; устаревшая эпоха — отказ.
   Для проверки, другой фазы, чужого эффекта или повторной привязки — отказ
   без изменения состояния и без эффектов.
3. Движок делает обычный детерминированный переход и устанавливает лизинг и
   работника. Он вычисляет адрес **следующего** `SendMutatingCommand` из
   чистого перехода подтверждения `DispatchArmedDurable`, учитывая переход
   привязки и проверяя переполнение счётчика. `CommandRef.transition_index`
   хранит этот адрес, а не адрес `PersistDispatchArmed`. Сам вход не является
   подтверждением durable arm и ничего не отправляет.
4. До постановки входа владелец операции сверяет ожидающий эффект, исходное
   полномочие, срок и удержанную выдачу с полным `ResourceOwner` и
   `ResourceSet`, включая режимы. Доверенный источник отдельно подтверждает,
   что выдача действительна. Номер операции или поля входа не являются
   полномочием. Глобальная блокировка не удерживается через внешнее ожидание;
   после ожидания контекст, срок и отзыв сверяются повторно.
5. После привязки `MutationArmPort` выполняет уже принятый ADR билета:
   сравнивает полный неизменяемый план с журналом и выдаёт неповторяемый билет
   только после подтверждённого `commit_durable`. Передача билета и отмена
   сериализуются владельцем; при неопределённом commit или потере после arm —
   сверка и карантин без повторной команды. Сам факт привязки не подтверждает
   живую выдачу, commit, физический покой или доверие к работнику.

#### Проверки после принятия

Покрыть регистрацию и удаление; точный прогноз адреса из `decide`; повторную
привязку; чужой вид операции, эффект, лизинг, worker и эпоху; переполнение
индекса; занятый актор; отзыв и истечение до и во время внешней проверки.
Затем связать проверенный вход с настоящим `MutationArmPort` и проверить
подмену плана, потерю ACK commit, потерю держателя и отсутствие повтора после
arm. Отдельно проверить целый путь с подставными источниками и явно обозначить,
что он не подтверждает аппаратного состояния или production-доверия.

<a id="mutation-dispatch-ticket"></a>

### Передача устойчивого билета изменяющей отправки

Порт arm сверяет неизменяемый план, ожидающий эффект, полномочие, выдачу, работника, эпоху и команду и выдаёт неповторяемый билет лишь после подтверждённого durable commit. Владелец сериализует передачу билета с отменой, отзывом и сроком; исполнитель потребляет его ровно одной отправкой после повторной сверки, включая прежнюю привязку и custody. Потерянное подтверждение arm не даёт билета. После устойчивого arm неопределённость и потеря владельца требуют удержания области и сверки по применимому профилю, без повторной команды.

#### Точный договор

1. Заменить в Runtime получение копируемого `ArmedMarker` вызовом отдельного
   порта, который принимает полную привязку операции, ожидающий эффект
   отправки, изменение, команду, работника, эпоху и область и выдаёт
   существующий неповторяемый билет только после `commit_durable`. Состав
   подтверждает ожидающий эффект, полномочия и живую выдачу у их владельцев;
   поля вызова сами по себе этого не доказывают.
2. Владелец операции удерживает билет и план до передачи исполнителю; билет
   потребляется по значению ровно одним вызовом изменяющей отправки. Отмена,
   отзыв, истечение срока или утрата согласованности до передачи запрещают
   вызов и сохраняют область заблокированной для предусмотренной сверки.
   Получение билета не является подтверждением покоя устройства.
3. Перед отправкой проверить полное совпадение билета, актуального актора,
   выдачи и плана, включая выбранную старую связку и вид custody для замены
   и удаления. Недоверенный провайдер не создаёт билет и не выбирает область.
   Публичный wire и формат кадров не менять.
4. Результат фиксации с потерянным подтверждением не выдаёт билет. Повторный
   arm или отправка после фактического `DispatchArmed` запрещены; владелец
   сохраняет блокировку, читает устойчивый снимок и передаёт состояние в
   восстановление. Только независимая проверка физического состояния может
   разрешить завершение сверки.
5. После начала отправки утрата владельца, отказ канала, отсутствие ACK или
   локальная неотправка не аннулируют уже устойчивый arm. Результат остаётся
   неопределённым, команда не повторяется, а область и ресурсы передаются
   предусмотренным владельцам сверки и карантина.

#### Проверки после принятия

Проверить неуспешный и потерянный после фактической фиксации commit; подмену
операции, перехода, команды, работника, эпохи, области и полной связки;
отзыв и истечение срока до и во время отправки; занятый актор; потерю
держателя до и после передачи; однократность при конкуренции и перезапуске.
Прогнать B-01/B-02, D-01…D-08 и B14-01…12 из спецификации. Тестовый
посредник не доказывает действующую аппаратную выдачу или физическое состояние.
