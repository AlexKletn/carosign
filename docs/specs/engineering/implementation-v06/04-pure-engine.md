# 04. `carosign-engine`: deterministic transitions and effects

**No I/O**: engine is a pure state machine. It does not read clocks, files, random numbers or sockets and knows nothing about protobuf. Runtime supplies prevalidated observations, generated IDs, timestamps and authorizations. Engine may reject inconsistent observations; it must not silently accept them.

## Primary interface

```rust
pub struct Decision<S> {
    pub next: S,
    pub effects: Vec<EffectSpec>,
}
pub fn decide(
    state: &OperationState,
    input: DomainInput,
) -> Result<Decision<OperationState>, TransitionError>;
```

`OperationState` is the normative tagged union specified in [28](28-v063-operation-discriminant-and-send.md): common context + `OperationBody::{Verification,Enrollment,Removal}`. Machine-specific phase and mutation plan belong only to the respective variant, never to a `mutation: Option<_>` selector. Every successful transition increments index by exactly 1. `EffectId` derives deterministically from `(OperationId, transition_index, effect_position)` **by a fixed keyed-free hash or explicit tuple persisted in metadata**; choose tuple on disk (`operation_id`, `transition_index`, `effect_position`) to avoid collisions and hidden hashing decisions. Runtime may assign a 16-byte transport `CommandId` when executing typed `EffectSpec::SendVerify` or `EffectSpec::SendMutatingCommand`, and records mapping in outbox.

## State machines stay separate

- `VerificationMachine`: biometric result, evidence check, completion.
- `EnrollmentMachine`: prepare/arm/send/confirm/registry/reconcile.
- `RemovalMachine`: same durable safeguards, generation-sensitive delete.
- `WorkerMachine`: spawn, handshake, bound, ready, draining, fault.
- `DeviceMachine`: resource state, uncertain, recover, unavailable.

The operation machine observes worker/device state via explicit `DomainInput`, **never** by reading another actor's mutable state. A ready worker with uncertain hardware cannot proceed.

## Verification transition table

| Current | Input | Next | Effects |
|---|---|---|---|
| `New` | `Authorized` | `WaitingLease` | `AcquireResources` |
| `WaitingLease` | `LeaseGranted` | `ReadyToDispatch` | `RevalidateGrant` |
| `ReadyToDispatch` | `GrantValid` | `Running` | `SendVerify` |
| `Running` | `ProviderProgress` | `Running` | `PublishProgress` |
| `Running` | `VerifyDispatchNotSent` | `Finalizing` | `RecordVerificationFailure(DispatchNotSent)`, `ReleaseResources` |
| `Running` | `VerifyDispatchUnknown` | `Cancelling` | `SuppressResult`, `RequestCancel`, `QuarantineResourceUntilQuiesced` |
| `Running` | `ProviderNotMatched` | `Finalizing` | `ReleaseResources` |
| `Running` | `ProviderMatched(evidence_ref)` | `EvaluatingEvidence` | `VerifyEvidence` |
| `EvaluatingEvidence` | `EvidenceAccepted` | `Finalizing` | `RevalidateOwner`, then `DeliverAccepted` only after valid |
| `EvaluatingEvidence` | `EvidenceRejected` | `Finalizing` | `DestroyEvidence`, `ReleaseResources` |
| Any nonterminal | `OwnerRevoked/DeadlineExpired` | `Cancelling` | `RequestCancel`, `SuppressResult` |
| `Finalizing` | `CleanupConfirmed` | Completion sequence below | Record the outcome and deliver as authorized by the applicable completion path |

`ProviderMatched` is **never** a terminal public success. If `OwnerRevoked` races with `EvidenceAccepted`, suppression takes precedence. Any stale epoch or worker instance event is rejected without changing state. `NoUsableSample` and failures enter finalizing with their distinct outcome; no implicit retry without an explicit configured retry policy controlled by Runtime.

## Mutation state machine (Enroll / Remove)

| Current | Input | Next | Effects |
|---|---|---|---|
| `New` | `ManageGrantValid` | `Preparing` | `PersistIntent` |
| `Preparing` | `IntentDurable` | `WaitingLease` | `AcquireResources` |
| `WaitingLease` | `LeaseGranted` | `Preflight` | `RevalidateGrant` |
| `Preflight` | `GrantValid` | `Arming` | `PersistDispatchArmed` |
| `Arming` | `DispatchArmedDurable` | `InFlight` | `SendMutatingCommand` |
| `InFlight` | `ProviderProgress` | `InFlight` | `PublishProgress` |
| `InFlight` | `ProviderCompleted` | `ValidatingChange` | `CheckProviderResult` (and independent query if required) |
| `ValidatingChange` | `ChangeConfirmed` | `Committing` | `CommitRegistryAndJournal` atomically |
| `Committing` | `MetadataDurable` | `Finalizing` | `ReleaseResources` |
| `Finalizing` | `CleanupConfirmed` | `Terminal` | `MarkCompletedAndNotify` |
| `InFlight/ValidatingChange/Committing` | `WorkerLost/Timeout/Unknown` | `Reconciling` | `PersistNeedsReconciliation`, `QuarantineScope` |
| `Reconciling` | `DesiredStateVerified(verified)` | `Committing` | `CommitDesiredRegistryAndJournal(verified)` atomically |
| `Reconciling` | `PreStateVerified(verified)` | `Committing` | `CommitNoChangeAndJournal(verified)` atomically; Registry contents unchanged |
| `Reconciling` | `VerificationIndeterminate` | `Reconciling` | `PersistNeedsReconciliation` if not yet durable; retain scope lock |
| `Committing` | `MetadataFailed` (after arm) | `Reconciling` | `QuarantineScope`, `RecoverMetadataCommit` (no new hardware mutation) |

**Nonnegotiable order:** durable intent before resource-side change; `DispatchArmed` durable before mutating command dispatch; Registry+Journal commit in one metadata transaction. Effects have explicit receipts. If crash occurs after `DispatchArmed`, even before actual send, recovery must conservatively treat hardware as potentially changed. **No automatic retry of mutating send.**

`CancelRequested` before `DispatchArmed` → stop and confirm no dispatch; after arming → request cancel and mark uncertain unless provider independently proves no side effect. `CancelReceived` is only an acknowledgement, not safe cleanup.

## Effects and receipts

| Effect | Confirmation input |
|---|---|
| `PersistIntent` | `IntentDurable` |
| `PersistDispatchArmed` | `DispatchArmedDurable` |
| `AcquireResources` | `LeaseGranted` / `LeaseDenied` |
| `RevalidateGrant` | `GrantValid` / `GrantRevoked` |
| `SendVerify` | `VerifyDispatchAccepted` / `VerifyDispatchNotSent` / `VerifyDispatchUnknown` |
| `SendMutatingCommand` | `MutationDispatchAccepted` / `MutationDispatchNotSent` / `MutationDispatchUnknown` (**all post-arm failures require reconciliation**) |
| `VerifyEvidence` | `EvidenceAccepted` / `EvidenceRejected` |
| `CommitDesiredRegistryAndJournal` | `MetadataDurable(Committed)` / `MetadataFailed` |
| `CommitNoChangeAndJournal` | `MetadataDurable(NoChange)` / `MetadataFailed` |
| `ReleaseResources` | `CleanupConfirmed` / `ResourceUncertain` |
| `DeliverOutcome` | `OwnerDeliveryRecorded` / `OwnerDisconnected` |

Engine never synthesizes a receipt for an I/O operation. Every input carries current op ID, device epoch and worker instance when applicable. Effects are idempotent **only where specified**: e.g. `PublishProgress` may coalesce; `SendMutatingCommand` is **not retryable**.

## Replay and deterministic test vectors

Replay uses persisted state/receipts to rebuild decision state, not to infer real hardware state. Incomplete mutation after `DispatchArmed` enters `Reconciling` and holds the affected enrollment scope locked. Test fixtures must include: crash before `IntentDurable`, after `IntentDurable`, after `DispatchArmed`, after worker response, after Registry CAS; duplicate/stale terminal; revoke during proof check; device epoch change; journal corruption.

### Binding clarifications B-01/B-02 (v0.6.2)

`Reconciling` never transitions directly to `Terminal`. Verified desired state and verified unchanged pre-state **both** pass through `Committing → Finalizing → Terminal`, with distinct atomic metadata effects. `OperationState` carries a pending terminal mutation outcome (Committed or NoChange), which is not publicly delivered before durable finalize and safe cleanup. `PreStateVerified` demands independently verified *complete* pre-state, including preexisting template during replacement; absence of only a new template is insufficient. Registry unchanged does **not** mean skip metadata transaction: journal/outbox and `MUTATION_NO_CHANGE` must be persisted atomically; scope lock remains until safe cleanup and `MUTATION_COMPLETED` with a durable `terminal_outcome`.

`MetadataFailed` after `DispatchArmed` **always** prevents success and triggers `Reconciling`/quarantine, including an ambiguous DB commit receipt. A failed storage write can prevent persisting `NEEDS_RECONCILIATION`; then mark in-memory storage unavailable and refuse operations, recover by reading durable arm/outbox state once storage returns. Never treat a lost metadata ACK as proof CAS did not commit. Recovery may detect Registry already committed but must validate it before finalizing; no repeat hardware mutation.

Verification has **no mutation `DispatchArmed`**; `SendFailedBeforeArm` is removed as a semantic input. `VerifyDispatchNotSent` is accepted only with a validated effect-runner receipt proving no command was dispatched. Transport send/ACK ambiguity is `VerifyDispatchUnknown`: no accepted factor may be delivered, and resource release awaits verified quiescence. Internal failure is typed (`DispatchNotSent` or `DispatchUnknown`); public mapping can be `Unavailable` for a transport/device outage, never `NotMatched`. See [27](27-v062-engine-conflict-resolutions.md).

### v0.6.3 operation classification and dispatch receipts

`decide()` routes solely through the `OperationBody` enum; `is_mutation_request(state)` is prohibited. An incompatible domain input (including Verify dispatch receipt in Enrollment/Removal) returns `InvalidInputForOperation` without effects or transition increment. `SendVerify` and `SendMutatingCommand` have disjoint receipt classes; generic `SendAccepted` is deprecated and cannot be used for routing. Exact semantics and acceptance cases: [28](28-v063-operation-discriminant-and-send.md).

### v0.6.4 — no generic matching stages

`Running` accepts optional semantic progress; there is **no mandatory capture, feature extraction, scoring or matching phase in Engine**. `ProviderMatched` is a validated provider observation only after authorized enrollment-scope validation in Runtime; Evidence is assessed separately. Never implement a Core matcher or template algorithm. See [29](29-v064-provider-owned-matching.md).

### v0.6.5 enrollment authorization sub-phases

`Enrollment` (only) has pre-mutating `AwaitingChallenge` and `AwaitingCredentialAuthorization` subphases when capabilities specify HAT. They are deterministic and accept trusted `ChallengePrepared`/`CredentialAuthorized`/failure observations. They are not provider capture/matching phases; no HAT bytes or challenge signing occurs in Engine. Only after trusted authorization and grant revalidation may the machine request `PersistDispatchArmed`; `SendMutatingCommand` requires durable arm as before. HAT-free providers skip these subphases. Any state/input mismatch is `InvalidInputForOperation` with no effects or transition increment. After arm, timeout/credential loss/worker loss results in reconciliation, not unverified NoChange. See [31](31-v065-enrollment-authorization.md).

## Завершение проверки

<a id="verification-terminal"></a>

### Запись терминального исхода проверки

Для проверки `RecordTerminal` фиксирует адрес эффекта и исход в ограниченной памяти владельца текущего запуска. `VerificationTerminalRecorded` отделена от `MetadataDurable` и мутационного finalize. Подтверждённая уборка предшествует записи; перед доставкой повторно проверяются полномочия. После перезапуска исход проверки не восстанавливается.

#### Вариант A

**A. Терминал проверки действует только в текущем запуске.** Принято, что
`RecordTerminal` для проверки означает фиксацию в ограниченном владельце
операции, а не запись на диск. Поток публикует терминальное событие только
при подготовке доставки после повторной проверки полномочий. Квитанция отдельна
от `MetadataDurable`; до неё требуется подтверждённая уборка, а до доставки —
свежая проверка полномочий. После перезапуска такую операцию нельзя
восстановить или предъявить как завершённую. Этот вариант не устанавливает
durability.

<a id="positive-verification-completion"></a>

### Завершение положительной проверки и доставка результата

Для `FactorAccepted` после уборки отдельный одноразовый владелец проверяет оценку, точный контекст, получателя, срок и отзыв и выполняет `DeliverOutcome`. Только подтверждённая запись полного пакета возвращает `OwnerDeliveryRecorded` ожидаемому эффекту. Неопределённая отправка не повторяется. Затем фактически уничтожается материал и фиксируется `RecordTerminal`; только после этого актор принимает `VerificationTerminalRecorded`. Подтверждённое отключение владельца подавляет успех и запускает уничтожение материала.

#### Точный договор

1. Расширить принятый вариант A на `FactorAccepted` только для текущего
   запуска. Запись имеет типизированный адрес эффекта и исход и не становится
   устойчивой, не расширяет `MutationPort` и не добавляет wire-поля.
2. `DeliverOutcome` исполняется отдельным одноразовым владельцем результата.
   До начала передачи он проверяет подтверждённую уборку, точный исход и
   контекст, независимую оценку и её область, действительность доверенного
   вызова и получателя, полномочия и срок. Повторная проверка и отправка
   согласуются с принятыми отзывом и отменой; фоновый наблюдатель остаётся
   действующим до фактической передачи или отказа.
3. После подтверждённой записи одного полного пакета исполнитель возвращает
   `OwnerDeliveryRecorded` ровно для ожидающего эффекта. Заполненный сокет
   сохраняет единственного владельца для новой проверки; неопределённая
   отправка не повторяется автоматически. Закрытие исходного владельца
   подавляет успех, а ответ и передача не подтверждают получение приложением.
4. После `OwnerDeliveryRecorded` или подтверждённого отключения владельца
   исполнитель уничтожает материал через его единственного владельца.
   `EvidenceDestroyed` допускается только после фактической очистки и возврата
   всех выданных копий, принадлежащих этому владельцу. Затем `RecordTerminal`
   фиксирует точный подтверждённый исход и адрес в памяти. Актор принимает
   квитанцию `VerificationTerminalRecorded` для положительного исхода только
   после этих переходов, без изменения существующей отрицательной ветки.
5. Отзыв, срок или отключение до записи пакета запрещают успех; после записи
   эти события не переопределяют отправленные байты, но не отменяют обязанность
   очистки. При ошибке уборки сохраняются ресурсы и карантин; при ошибке
   уничтожения запись завершения не подтверждается. После перезапуска
   операция не восстанавливается как доставленный успех.

#### Открытые зависимости

Выбор и проверка доверенной идентичности службы, источника контекста PAM,
политики и независимого проверяющего не входят в этот ADR. До их реализации
производственный `PAM_SUCCESS` остаётся недоступным. Тестовые проверяющий и
посредник не подтверждают аппаратную гарантию или физический покой.

#### Проверки после принятия

Настоящие парные `SOCK_SEQPACKET` и тестовые источники: успешная программная
оценка → ограждённая уборка → повторная проверка → один пакет → уничтожение →
терминал; отказ политики/отзыв/срок до и во время повторной проверки и перед
отправкой; `EAGAIN`, обрыв и неопределённый результат; повтор и чужой владелец;
ожидание освобождения материала у проверяющего; потеря исполнителя до и после
начала отправки. Отдельно проверить изолированный стек PAM только после
реализации доверенного контекста вызова и подтверждения подлинности службы.

<a id="positive-event-reservation"></a>

### Номер положительного события до завершения операции

`EventHub` резервирует один последовательный номер для точного `DeliverOutcome`, удерживая следующие номера без публикации терминала. При `EAGAIN` резерв остаётся у того же владельца. Неопределённая запись не повторяется и не освобождает номер. После подтверждённой доставки, очистки материала и терминальной записи хаб публикует маркер с зарезервированным номером. Безопасное повторное использование допустимо лишь при доказанном отсутствии начатой записи.

#### Точный договор

1. Добавить в `EventHub` ограниченное одноразовое резервирование **одного**
   номера следующего терминального события операции. Резерв создаётся только
   для точно адресованного ожидающего `DeliverOutcome` проверенной операции;
   номер и идентичность хаба не являются разрешением. Терминальный маркер не
   публикуется, в истории и очередях наблюдателей отсутствует. После резерва
   хаб не присваивает этот или последующие номера другим событиям до закрытия
   резерва; входящие управления при этом должны приниматься независимо.
2. Резерв удерживается единственным владельцем выдачи. Повторная политика,
   точная область оценки, доверенные вызов и получатель, срок и контроль
   отзыва проверяются независимо от наличия резерва. Отзыв или срок до записи
   подавляют успех. При `EAGAIN` резерв и результат остаются у того же
   владельца; при потере незаписанного владельца таблица возвращает их только
   исходному вызывающему после новой проверки.
3. Неполная/неопределённая попытка записи не повторяется автоматически и не
   превращается в подтверждение доставки. Резерв остаётся удержанным как
   неопределённый; хаб не выдаёт этот номер другой операции и не публикует
   успех. Восстановление без знания исхода не заявляет доставку. Падение
   процесса уничтожает этот внутрипроцессный резерв и не восстанавливает
   успешную проверку после перезапуска.
4. При подтверждённой полной записи `SOCK_SEQPACKET` актор получает ровно
   один `OwnerDeliveryRecorded` для связанного эффекта. Резерв остаётся в
   хабе до фактического уничтожения материала и подтверждённой терминальной
   записи текущего запуска. После них хаб атомарно публикует терминальный
   маркер с тем самым номером; до этого наблюдатели не получают терминал.
   Уведомления об отсоединённых наблюдателях удерживаются до принятия.
5. При доказанном отсутствии записи и отмене/отзыве резерв закрывается без
   публикации положительного терминала. Повторное использование его номера
   допустимо только если ни одна запись с этим номером не могла начаться;
   иначе номер остаётся занят до отказа/восстановления согласно пункту 3.
   Удаление или подмена хаба, операции, эффекта или владельца отвергается.
6. Публичные wire-поля и их значения не меняются. Это новый внутренний
   договор хаба и владельца результата; точные API и типы внедряются только
   после принятия ADR. Производственный источник идентичности, вызова,
   получателя и доверенный проверяющий остаются отдельными зависимостями.

#### Проверки после принятия

- Настоящий `SOCK_SEQPACKET`: положительная оценка → уборка → один пакет →
  уничтожение → терминальная фиксация; до фиксации наблюдатель не видит
  `Terminal`, после неё видит тот же номер, что получил владелец.
- Конкурирующие публикации, отзыв/срок/отключение до и во время повторной
  проверки, `EAGAIN`, потеря держателя, чужой владелец и повторная выдача.
- Сбой до send, короткий/неопределённый send, разрыв после записи, потеря
  подтверждения, переполнение `u64`; нет повторной выдачи или ложного успеха.
- Отзыв после подтверждённой записи не меняет отправленные байты и не отменяет
  очистку; перезапуск не восстанавливает результат как доставленный.
