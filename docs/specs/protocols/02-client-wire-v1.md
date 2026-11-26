# P2. `carosign.client.v1` — API операционной системы

## Назначение

Независимый интерфейс для PAM, GUI, CLI, сервисов аутентификации. Работает с авторизованными идентичностями и операциями, **не** с сырой аппаратной командой или биометрическим изображением.

## Начало соединения

`Hello { min_major, max_major, features } → HelloAck { chosen_major, chosen_minor, features, connection_id }`. До успешного handshake невозможны операции.

Linux peer binding: socket credentials ядра + запрос к policy authority + проверка жизненного цикла клиента; peer-provided username не принимается как факт. В Veles используется проверенный принципал/канал.

## Методы

| Метод | Принцип |
|---|---|
| `GetDevices` | возвращать только видимые caller устройства |
| `GetCapabilities` | отделять features от assurance assessment |
| `ListEnrollments` | только для разрешённого subject |
| `BeginVerification` | permission + challenge context + operation ID |
| `BeginEnrollment` | отдельная привилегированная операция |
| `BeginRemoval` | отдельная привилегированная операция |
| `ObserveOperation` | scoped observer, sequence/cursor |
| `CancelOperation` | receipt, не гарантия quiesce |
| `GetOperation` | snapshot с server-side ACL |
| `GetHealth` | безопасная минимальная телеметрия |

`Identify` — отдельное optional API с особенно строгой политикой; не сводить автоматически к `Verify` без заданного субъекта.

## Минимальный wire sketch

```proto
syntax = "proto3";
package carosign.client.v1;
message Envelope {
  uint32 major = 1;
  uint32 minor = 2;
  bytes request_id = 3;
  bytes operation_id = 4;
  uint64 event_sequence = 5;
  oneof payload {
    Hello hello = 10;
    HelloAck hello_ack = 11;
    BeginVerification begin_verification = 20;
    BeginEnrollment begin_enrollment = 21;
    BeginRemoval begin_removal = 22;
    OperationAccepted operation_accepted = 30;
    OperationEvent operation_event = 31;
    OperationSnapshot operation_snapshot = 32;
    CancelOperation cancel_operation = 40;
    ObserveOperation observe_operation = 41;
    ProtocolError error = 90;
  }
}
message Hello { uint32 min_major = 1; uint32 max_major = 2; repeated string features = 3; }
message HelloAck { uint32 selected_major = 1; uint32 selected_minor = 2; repeated string features = 3; bytes connection_id = 4; }
message BeginVerification { bytes idempotency_key = 1; bytes subject_selector = 2; bytes authority_request_ref = 3; }
message BeginEnrollment { bytes idempotency_key = 1; bytes subject_selector = 2; bytes authority_request_ref = 3; }
message BeginRemoval { bytes idempotency_key = 1; bytes enrollment_ref = 2; bytes authority_request_ref = 3; }
message OperationAccepted { bytes operation_id = 1; uint64 initial_sequence = 2; }
message ObserveOperation { bytes operation_id = 1; uint64 after_sequence = 2; }
message CancelOperation { bytes operation_id = 1; }
message OperationEvent { string kind = 1; bytes data = 2; }
message OperationSnapshot { bytes operation_id = 1; uint64 sequence = 2; string state = 3; }
message ProtocolError { string code = 1; }
```

Этот sketch использует placeholders (`bytes` для сложных структур, `string` для enum); **не считать готовой схемой API**. При нормировании заменить строгими protobuf messages и enum, закрепить presence и пределы размера.

## Владелец и наблюдатель

`OwnerHandle` контролирует lifetime и cancel privileges. `ObserverHandle` только подписывается на progress/result, если ACL разрешает. Отключение observer не отменяет операцию. Отключение owner отзывает authorization, но не означает rollback аппаратной мутации.

## Idempotency

Server хранит `(authority, principal, scope, idempotency_key, request_hash) → OperationId`. Повтор с той же нагрузкой даёт ту же operation, с другой — `IDEMPOTENCY_CONFLICT`. TTL и durable retention зависят от вида операции; незавершённая mutation живёт до reconciliation независимо от client lifecycle.

## События

`event_sequence` локален каждой операции, строго возрастает. Resume с `after_sequence` даёт сохранённые events либо `EventGap` + актуальный snapshot. Не обещать бесконечную историю. Terminal outcome mutation сохраняется достаточно долго для подтверждения уполномоченным владельцем.

## Исходы и секреты

`Matched` от worker не публикуется как разрешение на `sudo`. Runtime выдаёт результат только при допустимом контексте и принятых evidence; сам PAM/Authentication Authority применяет политику. Пароль/keyring secret **не** должен приходить через публичный core API как побочный эффект матча (специализированная secrets integration отдельна).

## Ошибки

`ProtocolError`: bad message/version/permission/resource budget. `OperationFailure`: no sample/busy/hardware/error/expired. `MutationOutcome`: committed/nochange/indeterminate/reconciliation. `NotMatched` — нормальное завершение проверки, не протокольная ошибка.
