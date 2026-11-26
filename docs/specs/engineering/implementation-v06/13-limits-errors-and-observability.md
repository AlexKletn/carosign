# 13. Bounded execution, error model and observability

## Baseline v1 limits (defaults; security ceilings)

| Parameter | Default | Absolute ceiling in v1 | Failure behavior |
|---|---:|---:|---|
| Protobuf control frame serialized bytes | 65536 | 65536 | close on oversized packet |
| Ancillary FDs per frame | 8 | 8 | close all received FDs, reject |
| Opaque provider enrollment ref bytes | 4096 | 4096 | `INVALID_MESSAGE` |
| Client connections concurrently | 32 | 128 | refuse new connection |
| Active+queued ops per connection | 4 | 16 | `LIMIT_EXCEEDED` |
| Queued ops per authenticator | 16 | 64 | `BUSY` |
| Observers per operation | 8 | 32 | `LIMIT_EXCEEDED` |
| Event history ring per operation | 128 | 1024 | explicit gap + snapshot |
| Worker actor command mailbox | 64 | 256 | backpressure/failure |
| Labels, UTF-8 bytes | 64 | 64 | `INVALID_ARGUMENT` |
| Public free-text error bytes | 256 | 256 | sanitize/truncate without secrets |
| Public Hello deadline | 5 s | 5 s | disconnect |
| Provider Hello/Initialize per phase | 5 s | 5 s | terminate worker |

Verification/enrollment *physical* timeouts cannot be universally fixed without device facts. Baseline policy is verify 30s, enroll 120s **subject to platform policy and device capability**, never unlimited unless explicitly authorized diagnostic mode, which is not public IPC. Cleanup timeout default 10s; expiration leads to quarantine/uncertain, not assumed quiescence. Configuration can tighten limits and may increase only within ceiling with explicit owner review.

## Errors are typed; no opaque `Error(String)` as protocol semantic

| Class | Codes (baseline) | Meaning |
|---|---|---|
| Protocol | `INVALID_MESSAGE`, `UNSUPPORTED_VERSION`, `UNEXPECTED_PHASE`, `FRAME_TOO_LARGE`, `FD_MISMATCH` | transport/schema violation |
| Authorization | `UNAUTHORIZED`, `GRANT_REVOKED`, `SCOPE_MISMATCH`, `SUBJECT_UNKNOWN` | policy boundary |
| Operation | `BUSY`, `UNSUPPORTED`, `UNAVAILABLE`, `TIMEOUT`, `NO_USABLE_SAMPLE`, `CANCEL_REQUESTED` | execution status |
| Persistence | `STORE_UNAVAILABLE`, `CORRUPT_STATE`, `GENERATION_CONFLICT`, `MUTATION_INDETERMINATE`, `RECONCILIATION_REQUIRED` | durable state |
| Evidence | `EVIDENCE_INVALID`, `EVIDENCE_UNVERIFIED`, `RECIPIENT_MISMATCH` | proof checking |
| Infrastructure | `WORKER_LOST`, `RESOURCE_UNCERTAIN`, `DEVICE_REMOVED`, `BROKER_FAILED` | process/device issues |

`NotMatched` is a verification outcome (normal), **not** a protocol/system error. `MutationMayHaveOccurred` cannot be rewritten to `Cancelled`/`NoChange`. Client-facing messages use stable error codes and optional sanitized diagnostic text, not raw stack traces/TA hex buffers.

## Observability and privacy

Structured logs contain timestamp, operation ID (where privacy policy permits), provider instance ID, phase, outcome code and timing bucket. No raw frames, template bytes, key/PSK, token, password, subject display names or full hardware serial. Sensitive diagnostic capture is **separate tool with explicit temporary authorization**, disabled by default and never exported to public UI. Audit event stream has its own retention policy; event hub is not audit log. Telemetry cardinality bounded; no uncontrolled label string values.

## Health vs authorization

`ServiceReady`: daemon listens and safe policy established. `DeviceAvailable`: per-device status. `EvidenceVerified`: only trusted verifier result. Do not confuse one with another. Service may be running while one provider is quarantined. Public health snapshot gives broad reason; privileged diagnostics can inspect details.

## Fault policy

- Invalid packet: fail current connection, preserve global state.
- Worker crash during verification: revoke result, request hardware recovery.
- Worker crash after mutating command armed: durable `NeedsReconciliation`; block affected scope.
- DB corruption: affected scope/entire store unavailable; never initialize empty state silently.
- Authorization/evidence service timeout: fail closed; no trust downgrade.
- No provider installed: healthy service with zero available authenticators.

### Dispatch/metadata error distinction (v0.6.2)

`VerifyDispatchNotSent` means verified locally that the provider command was not dispatched; its internal terminal result is `Failed(DispatchNotSent(reason))`. A service/device/transport outage may map to public `Unavailable`; unsupported operation or denied grant retain their own distinct public errors. `VerifyDispatchUnknown` indicates the command may have been delivered, invalidates any current proof and requires worker/device quiescence or quarantine. Neither becomes `NotMatched`.

`MetadataFailed` after `DispatchArmed` is never a harmless validation failure or `NoChange`; it enters reconciliation. Failure to persist that uncertainty is a *storage availability* fault, requiring an in-memory quarantine until the durable record can be reread. Do not mark durable recovery as successful from an unsuccessful write or an unknown commit ACK.

### v0.6.5 limited bulk and enrollment challenge

`broker.Call/CallResult.external_body` is broker-only protocol minor 1: **one** immutable sealed memfd, 1..8MiB and no more than handler-specific configured maximum; not counted in the <=64KiB serialized control envelope, but counted against bounded broker in-flight memory/FD/resource budgets. Challenge size 1..64B and ref 16B; trusted credential authority controls monotonic expiry. A broker-handler timeout with unknown DMA/TA completion retains ResourceUncertain; an armed mutation retains NeedsReconciliation. Log neither HAT nor challenge secret/token blob, raw hardware body, biometric data or DMA addresses.

## Места активных операций на соединении

<a id="connection-operation-limits"></a>

### Учёт операций на клиентское соединение

Реестр атомарно учитывает принятые и ожидающие операции всех видов `Begin` при публикации. Исчерпание даёт `LIMIT_EXCEEDED` без создания операции. Повтор на том же соединении не занимает второе место. Авторизованный повтор активной операции на другом соединении атомарно занимает там одно место; завершённая операция места не требует. Подтверждённый терминал однократно освобождает места. Закрытие соединения освобождает только его учёт. До отдельного административного подтверждения потолок — четыре места на соединение. Очередь кадров ограничивается отдельно, а заполненная очередь аутентификатора даёт `BUSY`.

#### Точный договор

1. Лимит применяется к операциям, принятым или ожидающим исполнения в рамках
   данного соединения, включая все виды `Begin`. Местом владеет операция, а не
   кадр и не число вызовов разбора. Уникальный идентификатор соединения не
   является полномочием. Решение о доступе к операции остаётся за identity и
   policy; учёт не заменяет их проверки.
2. Владелец реестра атомарно проверяет свободное место и связывает его с
   создаваемой операцией при её публикации. Параллельные запросы не могут
   превысить предел. Непринятые запросы, в том числе отказы полномочий,
   некорректные данные и сбой до публикации, не занимают место. Если места нет,
   вернуть типизированный `LIMIT_EXCEEDED` без создания операции или отправки
   команды провайдеру. Внешние вызовы и ожидания не выполняются под блокировкой
   учёта. Отдельная ограниченная очередь входящих кадров необходима для защиты
   ресурсов до публикации операции; она не считается числом операций.
3. Разрешённый повтор существующей операции по ключу идемпотентности не создаёт
   вторую операцию и не занимает второе место в том же соединении. При повторе
   на другом соединении до ответа проверяются полномочия; если операция ещё
   активна или ожидает исполнения, она учитывается один раз и на этом соединении.
   Решение об учёте и выдача ссылки атомарны: при полном лимите соединения
   ответ — `LIMIT_EXCEEDED`. Это предложение о границе соединения, а не право
   наблюдать чужую операцию. Повтор уже завершённой операции места не занимает.
4. После подтверждённого терминального состояния операция освобождает связанные
   с ней места однократно, независимо от доставки результата. Отказ или отмена
   запроса сами по себе не подтверждают терминальное состояние и не освобождают
   место преждевременно. Дублирующиеся уведомления о завершении безопасны.
   Состояния уборки и карантина оцениваются по договору операции, а не считаются
   терминальными по предположению.
5. При закрытии соединения уничтожается только его учёт; операция и полномочия
   управляются своими правилами. В частности, отключение наблюдателя не
   отменяет операцию. Закрытое соединение не восстанавливается под тем же
   идентификатором и не может освободить место другого соединения.
6. Настроенный предел применяется при создании сеанса; ноль и значение выше
   потолка отвергаются. Значения выше четырёх требуют явного решения владельца,
   и простой разбор числовой конфигурации не является подтверждением такого
   решения. Пока механизм подтверждения отсутствует, применять значения выше
   четырёх нельзя. Ограничение очереди аутентификатора — отдельный счётчик;
   по документу 13 его отказ обозначается `BUSY`.

#### Проверки после принятия

Пять одновременно принятых операций на одном соединении при пределе четыре:
пятая получает `LIMIT_EXCEEDED`, провайдер не вызывается. Серия отклонённых
`Begin` не расходует места; завершение освобождает место, повторный терминальный
сигнал не освобождает чужое. Проверить конкурентное создание, отказ до
публикации, отмену после публикации, отдельные соединения, idempotent replay,
закрытие соединения при работающей операции и конфигурацию 1, 4, 5, 16 и 17.
Проверить настоящий `SOCK_SEQPACKET` и отличие ошибки лимита соединения от
`BUSY` очереди устройства. Эти испытания не доказывают аппаратную уборку.
