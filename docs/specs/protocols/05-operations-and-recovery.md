# P5. Длительные операции, мутации и reconciliation

## Operation lifecycle

`Created → Authorizing → Queued → Acquiring → Running → Finalizing → Terminal`.

Отдельная ветка `CancellationRequested`; она не означает конечное состояние. Каждая операция содержит deadline, owner, operation ID, authenticator epoch, subject binding и current authority grant. Runtime — единственный источник истины о phase операции.

## Проверка

`Verification` не должна изменять Registry. После начала Runtime передаёт context, provider отвечает progress и `Matched(evidence)` или `NotMatched/NoSample/...`. Ошибки транспорта, отмена, пересоздание устройства приводят к fail-closed. Поздний положительный результат от старого worker подавляется.

## Enrollment/removal — durable mutation

1. Authority grant проверен.
2. Создана запись `Prepared` в журнале, устойчиво сохранена.
3. Подготовлен resource lease и выполнена revalidation.
4. Runtime отправляет **потенциально изменяющую** команду; с этого момента при потере подтверждения принимается `Indeterminate`.
5. Provider/TA выполняет действие. Где возможно, Runtime требует независимого запроса подтверждения состояния.
6. Registry фиксирует изменение binding и generation.
7. Mutation Journal отмечает `Completed`; только после этого внешнему клиенту сообщается подтверждённый успех.

`ProviderConfirmed` может быть недостаточно для финальной фиксации, если API не доказывает persistent state.

## Mutation state

`Prepared`, `Executing`, `ProviderConfirmed`, `RegistryCommitted`, `Completed`, `NeedsReconciliation`. Состояния могут содержать дополнительные подфазы; кодирование persistent journal versioned.

При crash после `Prepared`, если достоверно доказано отсутствие dispatch, допускается `NoChange`. При crash после dispatch — `NeedsReconciliation`, даже если worker прежде не успел прислать `MutationStarted`.

## Алгоритм восстановления

1. На старте открыть журнал до допуска новых mutating commands.
2. Все записи, не имеющие terminal `Completed`, сопоставить с current device/TA generation.
3. Заблокировать конфликтующую область регистраций и аппаратный resource.
4. Вызвать независимый `QueryMutationState`/enumeration/reconciliation при разрешении безопасных проверок.
5. Если факт подтверждён — согласовать Registry, затем journal; если отсутствие подтверждено — записать no-change. Если доказательств недостаточно — сохранить `NeedsReconciliation`.
6. Никогда не повторять mutating TA call просто из-за timeout.

## Registry & provider consistency

Транзакция redb и изменение в TEE не становятся единой ACID-транзакцией. Journal координирует согласование с возможностью `Indeterminate`, не обещая невозможный двухфазный commit для closed hardware. При recovery невозможна автоматическая гарантия, если backend не умеет query/verify; нужен явный degraded mode и путь ручного исправления под авторизацией.

## Cancellation outcomes

`CancelledBeforeMutation`, `CancelledAndQuiesced`, `MutationMayHaveOccurred`, `CancellationUnsupported`. `CancelReceipt` подтверждает принятие просьбы, не её исход.

## Outbound result and acknowledge

Для завершённых verification событий retention ограничен временем операции. Для mutations terminal result и journal сохраняют до acknowledgement доверенным владельцем либо до явной политики retention, не нарушающей согласованность. Повтор delivery terminal result не должен повторить аппаратный вызов.

## Fault examples

| Сбой | Действие |
|---|---|
| Worker пропал во время verification | Reject result, revoke, quiesce или isolate |
| Worker пропал во время enroll | Durable `NeedsReconciliation` |
| Registry commit произошёл, journal commit не успел | Reconcile binding, finish journal, не повторять enroll |
| Device identity сменилась | Quarantine affected enrollments до сверки |
| Cancel потерялся | Запустить supervisor recovery, не считать device free |
| Storage corrupt | Fail closed для affected enrollment; не создавать пустую базу «как будто никогда не регистрировали» |

### v0.6.5 — new requirements

All new mutations persist immutable `MutationRecord.plan` schema v2 in the Prepared transaction (33). HAT-dependent Enroll uses audited pre-enroll challenge + credential authorization before arm; only then one armed mutating dispatch (31). Generic broker command bodies >64KiB use sealed memfd and trusted handler-owned kernel/DMA lifetime; failed/ambiguous hardware execution is not retryable (32). All previous `NeedsReconciliation`, NoChange versus Committed, and three-way replace semantics remain in force.

### v0.6.6 normative correction

The previous stored v2 reference remains historical. Current implementation requires storage v3, exact selected `old_target`, typed custody and Runtime-built `CommandPlan` with explicit Create/Replace. Provider `StartEnrollment` always gets a planned new enrollment ref, with old binding only for REPLACE. See [34](../engineering/implementation-v06/34-v066-enrollment-command-projection.md).
