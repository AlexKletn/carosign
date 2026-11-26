# P1. `carosign.provider.v1` — интерфейс Runtime ↔ Hardware Worker

**Статус:** семантическая проектная спецификация + неполный, но согласованный skeleton Protobuf. Все провайдеры — отдельные процессы; SDK необязателен.

## Роли

**Manager** создаёт процесс, знает package/profile/instance, назначает устройство и выдаёт разрешённые ресурсы. **Worker** выполняет только операции поддерживаемого аутентификатора. **Runtime** остаётся владельцем `OperationId`, Registry и пользовательской идентичности.

## Обязательные вызовы

| Команда | Назначение | Допустимая фаза |
|---|---|---|
| `WorkerHello` | wire negotiation | Starting |
| `Initialize`/`Initialized` | контекст запуска/limits | Negotiating |
| `BindDevice`/`DeviceReady` | привязка lease и hardware epoch | Binding |
| `Describe` | актуальные capabilities | Ready |
| `StartVerification` | biometric claim | Ready + lease |
| `StartEnrollment` | mutating registration workflow | Ready + durable intent |
| `RemoveEnrollment` | mutating delete | Ready + durable intent |
| `RequestCancel` | best-effort cancellation | Running |
| `QueryMutationState` | безопасная сверка, если доступна | Recovering |
| `Quiesce` | остановить активность | Ready/Running/Draining |
| `CloseDevice`/`Shutdown` | release/exit | Draining |

Опционально `Identify`, `Enumerate`, `Probe` — исключительно когда capabilities сообщают поддержку и платформа разрешила операцию. `Enumerate` может вернуть `Unsupported`, не выдавая ложный пустой набор.

## Минимальный protobuf sketch

```proto
syntax = "proto3";
package carosign.provider.v1;

message Envelope {
  uint32 protocol_major = 1;
  uint32 protocol_minor = 2;
  bytes worker_instance_id = 3; // exactly 16 bytes
  uint64 message_seq = 4;
  bytes command_id = 5;         // exactly 16 bytes for command
  bytes operation_id = 6;       // 16 bytes or absent on lifecycle
  uint64 device_epoch = 7;
  oneof payload {
    WorkerHello worker_hello = 10;
    Initialize initialize = 11;
    Initialized initialized = 12;
    BindDevice bind_device = 20;
    DeviceReady device_ready = 21;
    StartVerification start_verification = 30;
    StartEnrollment start_enrollment = 31;
    RemoveEnrollment remove_enrollment = 32;
    RequestCancel request_cancel = 33;
    QueryMutationState query_mutation_state = 34;
    CommandAccepted accepted = 40;
    ProviderEvent event = 41;
    OperationOutcome outcome = 42;
    ProviderFault fault = 90;
  }
}
message WorkerHello { uint32 major = 1; uint32 min_minor = 2; uint32 max_minor = 3; repeated string features = 4; }
message Initialize { bytes instance_id = 1; uint32 selected_minor = 2; bytes profile_ref = 3; }
message Initialized { bool ready_for_binding = 1; }
message BindDevice { bytes authenticator_id = 1; uint64 epoch = 2; bytes lease_ref = 3; }
message DeviceReady { bytes authenticator_id = 1; uint64 epoch = 2; }
message StartVerification { bytes verification_context = 1; bytes bounded_subject_ref = 2; }
message StartEnrollment { bytes mutation_id = 1; bytes enrollment_context = 2; }
message RemoveEnrollment { bytes mutation_id = 1; bytes provider_enrollment_ref = 2; }
message RequestCancel { bytes operation_id = 1; }
message QueryMutationState { bytes mutation_id = 1; bytes recovery_ref = 2; }
message CommandAccepted { bool accepted = 1; string rejection_code = 2; }
message ProviderEvent { string kind = 1; bytes data = 2; }
message OperationOutcome { string kind = 1; bytes data = 2; }
message ProviderFault { string code = 1; bool recoverable = 2; }
```

**Внимание:** поля `bytes verification_context`/`string kind` — временные placeholders для демонстрации и подлежат замене строгими `message`/`enum` до первой реализации совместимого `v1`. Этот фрагмент нельзя объявлять финальным `.proto`.

## Поведение при командах

Worker отвечает `CommandAccepted` либо `Rejected` на синтаксически и семантически допустимый запрос, затем progress и ровно один outcome при нормальной доставке. После `Accepted` аппаратный вызов **может быть** ещё не начат. Worker не имеет права выдумывать `Verified` или системное `Authorized`.

Все команды проверяют `worker_instance`, `device_epoch`, `lease`, разрешённую operation, текущий session actor и монотонность последовательности. Неверные instance/epoch → протокольная ошибка без обращения к железу.

## Cancellation и progress

`RequestCancel` принимается control task независимо от блокирующей task. Worker сообщает `CancelReceived`, затем отдельно outcome `CancelledAndQuiesced | CancellationUnsupported | MutationMayHaveOccurred | Completed`. Остановка future ≠ отмена ioctl/TEE. Время ожидания cancellation ограничено; при hang Supervisor изолирует worker и переводит устройство в `Uncertain`.

Events `PlaceFinger`, `MoveFinger`, `RemoveFinger`, `CaptureProgress`, `RetryRequired` — UX semantics, не сырые биометрические кадры. Provider-specific telemetry доступна только под явным permission в диагностическом интерфейсе и не используется для принятия решений по безопасности.

## Mutating operations

Manager **до dispatch** фиксирует durable intent. Если команда отправлена в worker, failure/EOF считается `Indeterminate` до подтверждения через provider-specific reconciliation; нельзя полагаться на событие `MutationStarted` для safety. `ProviderConfirmed` может требовать дополнительного независимого query; неподдерживаемый query не даёт права автоматически признать успех.

## Capabilities

Descriptor включает: modalities, операции, template ownership, enrollment limits, capture interactions, cancellation guarantees, query/reconcile mechanisms и format/version IDs. В descriptor **нет** самоприсваиваемого trusted hardware level. Различать declared capabilities и signed/validated system assessment.

## Примеры отказов

- Unsupported hardware: `DeviceFailed(UNSUPPORTED)` до `Ready`.
- Already active session: `Rejected(BUSY)` без побочных действий.
- Lost IPC during verify: Runtime invalidates result and quiesces device.
- Lost IPC during enroll/remove: journal → `NeedsReconciliation`.
- Worker restart: новый instance/epoch, прежние replies reject.

## Поддержка любых языков

Worker может быть написан на C/C++, Rust, Go, Python и т. д. Он обязан реализовать protocol/framing/lifecycle, но не обязан ссылаться на Rust library. SDK — только удобная оболочка.

### Mandatory wire correction v0.6.4

Current normative schema in `contracts/proto/carosign/provider/v1/provider.proto` requires protocol minor **1**, since `StartVerification.permitted_enrollments` is mandatory nonempty and `EnrollmentBinding` models provider-opaque versus scoped-vault handles. Without this scope a legacy `v1.0` worker cannot be considered compatible. Matching strategy is private to provider and optional progress/capture hints do not create a generic matching pipeline. `VERIFY_MATCHED` with no token is a provider assertion (only if authorized scope), not hardware proof. See [29](../engineering/implementation-v06/29-v064-provider-owned-matching.md).

### v0.6.5 — current provider 1.2 baseline (overrides old illustrative sketch)

The authoritative wire is **`contracts/proto/carosign/provider/v1/provider.proto`**, not the embedded historical pseudoproto above. Provider major 1, selected minor **>=2** is mandatory for active operations. Provider-owned matching, permitted enrollment bindings and scoped removal binding remain required. Enrollment authorization is capability-dependent: HAT-required provider sends a pre-enroll challenge and later receives only a protected authorization **reference**, never raw HAT; detailed contract [31](../engineering/implementation-v06/31-v065-enrollment-authorization.md). Large hardware bodies use separate mediated broker 1.1 `Call.external_body` sealed memfd [32](../engineering/implementation-v06/32-v065-bulk-command-buffer.md), not provider-control fragmentation. No generic host matcher or template decoder.

### Current v0.6.6 baseline correction

The earlier illustrative v1.1/v1.2 fragments above are historical. The checked-in [`provider.proto`](../contracts/proto/carosign/provider/v1/provider.proto) requires negotiated **minor >=3** for live operations and mandatory `StartEnrollment.planned_enrollment` for create/replace. See [34](../engineering/implementation-v06/34-v066-enrollment-command-projection.md). Do not implement the old illustrative WorkerHello as a normative version configuration.
