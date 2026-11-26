# P0. Транспорт и framing v1

**Статус:** proposed Linux wire baseline; не единственный возможный транспорт всей платформы.

## Выбор

Linux: `AF_UNIX + SOCK_SEQPACKET + Protobuf`. Один datagram/packet = один закодированный `Envelope`; не добавлять `u32 length` как в старом GXFP IPC. Linux-specific FD-передача — `SCM_RIGHTS`, peer metadata — `SO_PEERCRED`/дополнительные проверки. Veles использует семантически аналогичные сообщения через собственный adapter; совместимость транспортного framing там не обязательна.

## Каналы

- Client endpoint: системный `/run/carosignd/control.sock` с контролем подключения, привилегий и per-operation scope.
- Provider endpoint: приватный `socketpair`, создаваемый Supervisor для одного worker. Worker **не** слушает публичный сокет.
- Broker/Interaction/Evidence internal: in-process Rust calls или приватные RPC, когда требуется process boundary.

## Framing

1. Размер packet payload в control plane ≤ **65536 bytes** (предлагаемый default, включая envelope). Большие объекты — дескриптор отдельного ограниченного blob, не Protobuf-поле произвольной длины.
2. При чтении проверять `MSG_TRUNC`, `MSG_CTRUNC`, общий размер и число FD. При усечении отклонять всё сообщение, закрывать полученные FD.
3. Неизвестная необходимая feature/security semantics → `UNSUPPORTED_FEATURE` или protocol error; игнорирование неизвестных полей безопасно только для необязательных расширений.
4. Поля типа opaque ID валидируются по длине, версия — до выполнения команд, дублированный command_id проверяется.
5. Участник обязан ограничивать память, частоту, размер collections и глубину вложенности Protobuf независимо от объявленного лимита packet.

## FD transfer: полный контракт

FD-передача разрешена только для сообщений, где схема явно задаёт `FdSlot`/`fd_count`. Все FD в одном packet должны соответствовать дескрипторам и порядку в payload. При несоответствии числа, типа или прав полученные FD закрываются без выполнения операции. После успешной валидации получатель становится владельцем копий FD и обязан их закрыть.

Пример: `AttachResource { lease_id, slots: [fd_slot=0, kind=DEVICE] }` + `SCM_RIGHTS` с ровно одним FD. `lease_id` не даёт доступа без server-side lease registry и проверки принадлежности worker.

**Ограничение:** `SCM_RIGHTS` не предоставляет аппаратный отзыв. При необходимости сильного revoke используется mediated RPC/отдельный broker, поддержка ядра или остановка изолированного worker, а устройство остаётся недоступным до quiesce.

## Handshake

1. Worker посылает `WorkerHello(supported_major, min_minor, max_minor, features)`.
2. Manager проверяет идентичность **запущенного процесса**, а не присланное имя; выбирает совместимую версию.
3. Manager посылает `Initialize(instance_id, selected_version, profile_ref, limits)`; Worker подтверждает `Initialized`.
4. Только после `BindDevice` и подтверждённого resource lease разрешаются аппаратные операции.

Client аналогично выполняет `Hello/HelloAck`; до этого допустимы только handshake/errors. Major несовместим → `INCOMPATIBLE_VERSION`; minor-negotiation разрешает только общие возможности.

## Correlation и доставка

- `message_seq`: строго монотонен в рамках соединения; reset с новым worker instance.
- `command_id`: UUID-подобные 16 байт, идентифицирует один request/reply.
- `operation_id`: назначается Runtime и используется для событий/результата.
- `event_sequence`: возрастающий номер событий внутри операции (вне соединения), позволяющий resume/Gap.
- `CommandAccepted` = принятие обработки, **не** финальный результат.
- `OperationOutcome` ровно один для завершённого runtime execution; потеря ответа от worker не означает, что TA ничего не сделала.

## Сессии/время

Deadline определяется Runtime по monotonic boottime внутри загрузки; нельзя сериализовать `CLOCK_BOOTTIME` и трактовать значение как пригодное после reboot. Durable записи используют boot identity и state recovery. Все устаревшие verification grants инвалидируются после boot/worker/device generation change.

## Языковая нейтральность

Нормативны protobuf, состояния и wire timings. Rust traits/SDK, Tokio, GLib, Go goroutines и Python asyncio — implementation details. Не разрешается передавать layout memory structures языка без явной схемы, а также сериализовать внутренние rust enums как ABI.

## Открытые решения

Схемы transport fd metadata, механизм выбора process runtime, точный размер лимитов, устойчивый event spool, extension handling и runtime schema compatibility требуют финального conformance профиля.
