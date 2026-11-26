# 06. `carosign.provider.v1`: process and wire contract

Семантика расширенного описания аутентификатора и событий захвата определена
в [документе 36](36-authenticator-types-and-events.md).

**Authoritative schema:** `contracts/proto/carosign/provider/v1/provider.proto` + imported common. This document specifies runtime behavior the schema alone cannot express. Provider may be implemented in C, C++, Go, Rust, Python, etc. The Rust SDK is optional. No stable Rust ABI.

## Transport

- Linux: private `socketpair(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC)` created by supervisor. One whole serialized `ProviderFrame` protobuf per datagram/message; no additional four-byte length prefix. Max message **65536 bytes**, including serialized envelope, excluding ancillary data. No implicit fragmentation; larger payloads require negotiated FD transport.
- Provider-control FD passing uses `SCM_RIGHTS` *only* with `BindAuthenticator`. The separate mediated **broker** socket permits a tightly scoped sealed-memfd external body in protocol minor 1; never forward it through the provider-control channel. `fd_slots` matches ancillary descriptor order and exact count, maximum 8 FDs per frame. If count/kind mismatches, close all received FDs and reject frame without hardware side effects. Runtime and broker create FD, not arbitrary worker claims.
- Sequence `frame_seq` starts at 1 per direction and increments by 1. Gaps/duplicates produce protocol fault and close session; worker/epoch values are fenced on every operation frame.
- No public socket address for worker. Established process identity is anchored by supervisor's verified spawn and inherited channel, not text inside WorkerHello.
- Version: major must equal 1; select **highest common** minor between advertised ranges, but **reject any result below 2**: scoped enrollment bindings and challenge authorization require v1.2; minor 0/1 workers cannot execute live operations. `feature` IDs agreed by both; unknown feature IDs ignored only at negotiation, never assumed active. New security semantics require new major/explicit feature.
- Transport bytes are **never** blindly converted to trusted `Grant`/`Assessment`/`DeviceLease`. Runtime adapter validates scopes.

## Handshake and phases

| Phase | Runtime→Worker | Worker→Runtime | Failure |
|---|---|---|---|
| `Starting` | none | `WorkerHello` within 5s | terminate worker |
| `Negotiating` | `Initialize` | `Initialized` within 5s | terminate |
| `Binding` | `BindAuthenticator` with scoped resources | `AuthenticatorReady` within 15s (or profile-approved bounded override) | isolate and close leases |
| `Ready` | `Describe`, `Start*`, `QueryMutation`, `RequestCancel`, `Quiesce` | accepted / progress / terminal | operation-specific recovery |
| `Draining` | `Quiesce`, `CloseAuthenticator`, `Shutdown` | cleanup confirmation | quarantine on uncertain hardware |

Handshake packets outside valid phase are protocol errors. Worker does not open sensors during Hello; first broker-issued hardware access is only after policy-approved `BindAuthenticator`. An `AuthenticatorReady` response never certifies hardware safety by itself: manager still checks broker/assessment state.

## Operation messages

`StartVerification`: targets one resolved subject binding + authenticated `VerificationContext`. An outcome is provider-origin `Matched` (possibly without token, converted to scoped ProviderAssertion by trusted Runtime), `NotMatched`, `NoUsableSample`, `Cancelled` or typed failure. `Matched` is **not** authentication success.

`StartEnrollment`: mutating operation (including trustlet prep that may persist); must carry `MutationId` and prepared context. Worker must not initiate unjournaled persistence. `StartRemoval`: targets `(EnrollmentId, expected_generation, provider_opaque_reference)`; stale generation → reject without deletion.

`QueryMutation`: only safe, non-mutating reconciliation mechanisms, with explicit `Unsupported/ConfirmedPresent/ConfirmedAbsent/Unknown`; must **not** silently create or delete enrollment. If hardware lacks trustworthy query, return `Unknown`.

`RequestCancel`: accepts request anytime operation active; ACK `CancelRequested` is *not* terminal. Final outcome may be `CancelledAndQuiesced`, `Completed`, `MutationMayHaveOccurred`, `CancellationUnsupported`. Last two require reconciler/device recovery rules. Cancellation cannot block behind capture worker; control reader stays responsive.

`Quiesce`: request controlled stop. Device cannot become Available until broker or supported hardware adapter confirms physical safety. Process death alone is insufficient.

## Worker event sequencing

Every event binds current `operation_id`, `device_epoch`, `worker_instance` and increasing `event_sequence` **per operation**. Progress events can be coalesced for public clients; provider→runtime events cannot fabricate terminal success. Exactly one normal terminal `ProviderOutcome` per accepted command. Duplicated terminal → protocol fault; stale epoch → discard/quarantine event, no registry mutation.

`CommandAccepted` only confirms command acceptance, not device dispatch nor durability. If a mutating send was armed before transmission, a missing accepted/outcome is still indeterminate.

## Worker privileges

Manifest describes needs, but a root-owned policy authorizes capabilities. Provider sees only approved FDs/RPC services. No direct root, network or broad `/dev/*` by default. For QSEE, arbitrary TA/raw RPMB access forbidden to untrusted worker; use scoped broker API. For libfprint C/GLib, worker may load its runtime and initialize GLib loop inside sandbox. Provider package does not self-certify assurance.

## Version constraints

Provider protocol v1 must treat unknown enum values affecting operation semantics as invalid; optional unknown fields must not grant rights or change meaning of known fields. Reject unexpected payload for phase, invalid ID size, excessive repeats, unapproved FD slots and unknown result statuses. Wire schema is the only compatibility ABI; Rust SDK must be verified against independent fixture suite.

### Normative FD-slot contract (v0.6.1)

`BindAuthenticator` is the **only** provider-protocol message carrying `SCM_RIGHTS` in current v1.2, including for mediated-service sockets. Each `FdSlot` has `index`, `role`, `resource_id`. Exact ancillary fd count equals `fd_slots` length (0..8), indices form a permutation of `0..N-1` without duplicates, roles are known, resource IDs are nonzero 16B **and must appear in a broker-approved active `ResourceLease` for this worker and current authenticator epoch**. Compare the specific approved fd↔resource binding held by the broker, not merely a provider-provided string. A second slot with same resource ID requires an explicit distinct approved grant; otherwise reject. All failed binds close every received descriptor, report protocol fault and grant no device access. The worker must confirm actual FD shape, and the system validates it against broker-issued handle metadata.

Roles: `FD_ROLE_DEVICE` = specifically delegated approved device handle (not assumed to be only a char-device fd); `FD_ROLE_EVENT` = approved `eventfd` notification handle; `FD_ROLE_CONFIG` = read-only sealed `memfd` carrying bounded versioned configuration; `FD_ROLE_MEDIATED_SERVICE` = connected `AF_UNIX/SOCK_SEQPACKET` socket to **trusted** hardware broker speaking `carosign.broker.v1`. Broker validates lease, worker, resource, method allowlist and typed payload for **every call**; no arbitrary ioctl/TA command is exposed. The mediated service is **not** the provider control connection and cannot confer new capabilities. See [09](09-device-and-broker.md), [20](20-protobuf-wire-validation.md), [25](25-v061-conflict-resolutions.md) and `contracts/proto/carosign/broker/v1/broker.proto`.

### v0.6.4 — scope-bound verification for all implementation strategies

`StartVerification` now includes a bounded set of authorized `EnrollmentBinding` values. The worker handles matching internally (host, chip, TEE, libfprint) and may return `Matched` with no raw evidence token; Runtime creates a scoped ProviderAssertion instead of pretending a hardware credential exists. Reject unscoped matches; optional `matched_enrollment` must be in the granted binding set. `StartEnrollment` includes the old binding on replace, and `StartRemoval` is scoped to its expected binding. Protocol version 1.2 is required for the current baseline (including challenge authorization); 1.1 describes only the historical scoped-matching revision. See [29](29-v064-provider-owned-matching.md).

### v0.6.5: enrollment challenge and scoped removal (provider 1.2)

`CapabilityDescriptor.enrollment_authorization` MUST be `NONE` or `HAT_CHALLENGE` for an enrolling worker. HAT mode requires `PrepareEnrollmentChallenge → CommandAck → EnrollmentChallengeReady` followed by trusted credential step-up and `StartEnrollment` with the scoped authorization ref plus SHA-256 challenge digest; raw HAT is prohibited in provider control frames. Prep may run before durable arm **only** if the specific pre-enroll path is audited not to persist enrollment state. Details: [31](31-v065-enrollment-authorization.md).

`StartRemoval.enrollment_binding` is REQUIRED in provider minor 2 and must exactly match `enrollment`. When `PROVIDER_OPAQUE`, legacy `provider_opaque_ref` equals its opaque ref, or may be empty to avoid duplication; when `SCOPED_VAULT`, `provider_opaque_ref` MUST be empty. Every active/generation/subject scope is checked by Runtime; no implicit full-database access. `QueryMutationResult` fields are provider hints, **not independently verified hardware reconciliation**. See [33](33-v065-durable-mutation-plan.md).

### Provider protocol v1.3 (v0.6.6 current baseline)

The previous v1.2 wording is historical. Current provider live operations require negotiated **minor >=3**. `StartEnrollment.planned_enrollment` (field 9) is a mandatory system-assigned NEW `EnrollmentRef`, for both create and replace. Exactly one of the internally typed `EnrollmentCommandChange::Create|Replace` alternatives determines whether `replace`/`replace_binding` are absent or both present; the complete checked old binding is supplied to the provider adapter by Runtime's `CommandPlan`. On completed enrollment, `MutationOutcome.enrollment` echoes the planned new ref; echo is not a hardware proof. Full validation and storage v3 semantics in [34](34-v066-enrollment-command-projection.md).

## Транзакция регистрации и явный откат

Провайдер владеет транзакцией регистрации с момента начала захвата до
подтверждённого сохранения или отката. Он изолирует временное состояние,
сериализует сохранение с откатом и отвечает за восстановление исходного
состояния при откате, включая прежнюю регистрацию при замене. Конкретные
команды датчика, порядок сброса и проверка результата принадлежат провайдеру.

Владелец операции на платформе направляет явный запрос отката для точной
транзакции. Провайдер может подтвердить откат только после остановки захвата,
устранения временного состояния и проверки сохранности полного исходного
состояния. После подтверждения отката эта транзакция больше не может сохранить
шаблон. Повтор запроса относится к той же транзакции и не запускает захват
повторно. Квитанция о получении запроса, завершение процесса и сброс сеанса
сами по себе подтверждением отката не считаются.

Платформа проверяет происхождение результата и его привязку к операции,
мутации, команде, экземпляру провайдера, эпохе и точной выдаче. При
подтверждённом откате она фиксирует `NoChange` в журнале и освобождает область
после подтверждённой уборки выдачи. При неизвестном исходе сохраняет ограждение
и обращается к механизму восстановления провайдера. Платформа не выполняет
аппаратный откат и не повторяет регистрацию автоматически.

`FEATURE_ENROLLMENT_TRANSACTION_V1` объявляется в `WorkerHello` и выбирается
в `Initialize`. `EnrollmentTransactionRequest` содержит номер мутации,
номер исходной команды регистрации и действие `ROLLBACK` или `QUERY`.
Конверт сохраняет номер операции и использует отдельный номер команды.
`EnrollmentTransactionResult` отвечает на этот номер, повторяет номера
мутации и исходной команды и сообщает `ROLLED_BACK`, `COMMITTED` или
`UNKNOWN`. `CancelRequested` и `MutationNoChange` сохраняют свои значения.

В текущем Linux-составе посредник пропускает один `ROLLBACK` для начатой
регистрации в той же живой выдаче. `ROLLED_BACK` открывает путь `NoChange`
после уборки. `COMMITTED` без проверенного результата сохранения, `UNKNOWN`
и потеря ответа сохраняют область на сверке. `QUERY` зарезервирован для
отдельного проверенного сеанса восстановления после перезапуска. Пока этот
сеанс не подключён, исход из старого сеанса не принимается автоматически.
Проверки провайдера охватывают отмену при захвате, гонку с сохранением,
повтор запроса и потерю ответа или процесса.

<a id="linux-provider-rendezvous"></a>

## Подключение к независимо запущенному провайдеру Linux

### Публикация и доверие

Доверенная системная установка разрешает профиль и назначает ему отдельный
UID/GID. Профиль связывает имя публикации, пакет, профиль, допустимые
идентификаторы аутентификаторов и наборы аппаратных ресурсов. Общий каталог
`/run/carosign/providers/` защищён от записи провайдерами. У каждого
профиля есть отдельный каталог, подготовленный systemd или администратором,
с доступом для своего UID и платформы. Провайдер публикует
`/run/carosign/providers/<profile>/control.sock` типа
`AF_UNIX/SOCK_SEQPACKET`. Платформа проверяет защищённый путь без символьных
ссылок, тип узла и владельцев. После подключения обе стороны проверяют
`SO_PEERCRED` по системной конфигурации. UID провайдера выделяется одному
доверенному профилю. Подмена сокета и перезапуск требуют новой проверки.

При старте платформа перечисляет разрешённые публикации. Изменения каталога
инициируют повторную сверку; после потери событий сверка выполняется целиком.
Отказ, зависание и неизвестная публикация изолируются от других профилей.
Дубликат устойчивого ID считается конфликтом до проверки его происхождения.
Права на публичное чтение инвентаря проверяются по документу 07.

### Инвентарь и сеансы

Контрольный канал обслуживает ограниченное ожиданием рукопожатие,
перечисление аутентификаторов и открытие сеансов. Описание возвращает
устойчивый `AuthenticatorId`, декларацию возможностей и состояние
готовности. Платформа сверяет ID и ресурсы с разрешённым профилем; описание
провайдера само по себе не подтверждает доступ к ресурсам или права
клиентского запроса. Путь сокета, PID и номер сеанса не входят в устойчивый
ID. Платформа назначает новую эпоху при потере привязки или смене экземпляра
провайдера. События старой эпохи отвергаются. Переподключение разрешено после
повторной проверки профиля и идентичности.

Открытие сеанса привязано к одному аутентификатору, эпохе и свежему
`WorkerInstanceId` платформы. На каждую активную операцию демон провайдера
создаёт изолированного работника с отдельным приватным `SOCK_SEQPACKET`.
Платформа связывает этот канал с точным экземпляром работника до передачи
канала брокеру. Проверка `WorkerHello` и согласование возможностей идут
по штатному операционному протоколу.
У платформы должна быть наблюдаемая ядром ссылка на работника для остановки
и проверки выхода. Завершение установочного обмена не подтверждает
готовность и не даёт работнику аппаратных FD.

Операционный обмен брокера с работником сохраняет `carosign.provider.v1`.
Схема локального обмена описана в
[provider_rendezvous.v1](../../../../contracts/proto/carosign/provider_rendezvous/v1/rendezvous.proto).
Версия обмена равна 1. На контрольном соединении допустим ровно один
`Request` и один `Response` с тем же ненулевым 16-байтовым ID запроса.
Кадры ограничены 64 КиБ, IPC-дескрипторы в них запрещены. Обмен использует
каноническое кодирование полей Protobuf по возрастанию номера без
неизвестных полей, дубликатов и явно записанных значений по умолчанию. `Inventory`
ограничен 64 записями. Переполнение, неверная версия, неизвестный ответ,
повтор идентификатора в том же соединении и неожиданный пакет дают отказ.

Для `OpenSession` платформа создаёт отдельный одноразовый слушатель
в своём защищённом runtime-каталоге. Полный путь обязан помещаться в
`sockaddr_un`. Демон провайдера передаёт путь и одноразовый 32-байтовый
nonce своему работнику; файлы ресурсов при установке сеанса не передаются.
Платформа проверяет UID/GID подключившегося работника и удерживает pidfd,
полученный ядром от этого соединения через `SO_PEERPIDFD`. Если возможности
ядра недостаточны, сеанс недоступен. Первый пакет работника — `WorkerProof`,
точно повторяющий ID запроса, аутентификатора, эпоху, ID работника и nonce.
После установки сеанса идёт штатный `WorkerHello`. Для допуска нужны как `SessionOpening`
по контрольному соединению, так и успешная привязка работника. Платформа
закрывает и удаляет одноразовый слушатель при успехе, ошибке или по сроку.
При успехе брокер получает принятый приватный FD и удержанную привязку
работника. После ошибки новый сеанс требует свежих ID и nonce.
Клиентский протокол остаётся прежним.

#### Закрытый канал владельца ресурса

Если профиль требует `resource_owner_worker_channel`, платформа выставляет
`OpenSession.owner_channel_required`. Демон провайдера передаёт этот флаг
работнику вместе с параметрами запуска. Для `declaration_only` флаг запрещён.

После проверки `WorkerProof` платформа передаёт по принятому worker-сокету
одним пакетом байт `0x00` и ровно один FD `AF_UNIX/SOCK_SEQPACKET`
через `SCM_RIGHTS`. Это сторона закрытой пары, вторая сторона которой
передаётся доверенному владельцу ресурса при запуске. Работник проверяет
полномочия платформы, тип и число принятых FD, выставляет `CLOEXEC` при
приёме и отправляет пакет `0x01` без дескрипторов. Затем идёт `WorkerHello`.
Установочные пакеты проходят в пределах срока `OpenSession`; отказ,
обрыв или неверный ответ прерывают запуск работника до подключения брокера.

Контрольный сокет демона провайдера сохраняет запрет на передачу FD.
Аппаратные FD выдаются брокером при `BindAuthenticator` после допуска
ресурса. Операционный `carosign.provider.v1` сохраняет прежние кадры.

### Ресурсы, отмена и перезапуск

В режиме `owner_managed` защищённый профиль назначает доверенного
владельца для эксклюзивного `ResourceId`. Платформа сохраняет ограждение для
полной области до отправки `Bind`. Брокер проверяет эксклюзивное требование к
этому ID и отправляет владельцу `Prepare` без слотов. Владелец захватывает
ресурс после `Prepare` и подтверждает ту же область без дескрипторов.
Отдельные слоты конфигурации брокер открывает по защищённому профилю.

Проба готовности требует успешной привязки и подтверждённой уборки по тому же
контракту. Освобождение требует `ExclusiveQuiesced` исходной области и
завершения процессов. Отмена, таймаут, потерянный ответ или несовпадение
области сохраняют ограждение, если уборку подтвердить не удалось. После
перезапуска такое ограждение восстанавливается из устойчивого хранилища.

Брокер и назначенный владелец ресурса сохраняют выдачу, ограждение и
подтверждение освобождения. Аппаратные FD выдаются только работнику при
`BindAuthenticator` после проверки лизинга. Долгоживущий демон провайдера
не получает аппаратные FD этого сеанса и не вправе удерживать их копии.
Закрытие сокета, останов работника и исчезновение публикации не подтверждают
аппаратный покой. При отсутствии подтверждения уборки действует ограждение
из документов 09 и 10, в том числе после нового подключения.

Отмена записи запрашивает явный откат у работника той же транзакции.
Готовность к следующей операции восстанавливается после подтверждённого
исхода и уборки. Отмена не удаляет аутентификатор из инвентаря. При потере
публикации аутентификатор становится недоступен, а регистрации остаются
привязаны к его ID. Неопределённая изменяющая операция сохраняется для
сверки; переподключение не запускает её повторно.

## Наблюдение результата проверки и запуск Linux

<a id="provider-verification"></a>

### Проверенный результат независимого провайдера

Один владелец сеанса читает кадры и проверяет последовательность, работника, эпоху, операцию, команду и единственность терминального ответа. Он сверяет указанную регистрацию и её поколение с разрешённым набором и передаёт типизированное наблюдение с видом и форматом доказательства. Конструктор наблюдения принадлежит проверке сеанса. Закрытая подписка и отправка команды остаются отдельными операциями порта.

#### Точный договор

1. Привести внутренний порт провайдера к договору документа 21: сеанс,
   отдельная отправка команды и ограниченная подписка событий. Внешний
   Protobuf и номера его полей не менять.
2. Сеанс единолично читает кадры, проверяет последовательность, экземпляр
   работника, эпоху, операцию, команду и единственность терминального ответа.
   Ссылка `matched_enrollment`, если присутствует, сверяется с разрешённым
   набором операции, включая поколение.
3. Передавать владельцу операции проверенное наблюдение, сохраняющее вид
   доказательства (`нет`, `ссылка`, `непрозрачный токен`), формат и ссылки.
   Закрытый конструктор доступен только проверке сеанса; данные работника не
   становятся полномочием или доверенным `EvidenceId`.
4. При отсутствии токена доверенный Runtime создаёт ограниченный контекстом
   `ProviderAssertion` и новый внутренний идентификатор доказательства.
   Политика с требованием аппаратной гарантии отклоняет такое утверждение.
   Токен направляется совместимому независимому проверяющему; он не попадает
   в публичный интерфейс клиента.
5. Применить тот же wire-контракт к отдельному моковому процессу и будущему
   провайдеру TB371FC. Мок не объявляет аппаратную гарантию. Код Goodix, TA и
   их зависимости не добавляются в workspace платформы.

#### Проверка

Отдельный процесс мока проходит полный обмен `Hello → Initialize → Bind →
StartVerification → Ack → Outcome`. Испытания покрывают совпадение с
регистрацией в наборе, чужую регистрацию и поколение, старую эпоху, другую
команду и экземпляр работника, повтор исхода, отсутствие токена и отказ
аппаратной политики для `ProviderAssertion`. Ни один ответ провайдера сам по
себе не выдаёт клиенту успех; отзыв и срок проверяются перед доставкой.

Это решение не определяет доверенный запуск работника, выдачу дескрипторов,
схему миграции БД или физическое подтверждение покоя: для сквозной готовности
эти границы также должны быть реализованы по соответствующим спецификациям.

<a id="linux-provider-supervisor"></a>

### Linux supervisor отдельного процесса провайдера

Linux supervisor запускает проверенный закреплённый объект в отдельном непривилегированном процессе с частным `SOCK_SEQPACKET`. Помощник ограничивает унаследованные FD, среду исполнения, привилегии и ресурсы до `exec`; наблюдает конкретный экземпляр процесса через pidfd. Подтверждение `exec` отделено от готовности по handshake. При ошибке или смерти выполняется уборка; неизвестный исход не повторяется. Требования к установке и допуску для первого выпуска определяет [документ 35](35-first-release-scope.md), включая его исключения для дополнительных проверок происхождения и cgroup.

#### Точный договор

1. **Доверенная установка.** Привилегированный установщик, управляемый
   системным пакетным менеджером, создаёт принадлежащий root каталог активной
   версии и отдельную root-owned policy. Недоверенный процесс не может изменять
   или переименовывать ни один из компонентов пути, манифест, профиль,
   исполняемый файл, зависимости среды запуска или policy. Policy связывает
   нормализованный package id и версию с проверенным пакетным артефактом,
   относительными путями, профилем, выделенным UID/GID класса, seccomp-фильтром
   и лимитами. Текст манифеста сам по себе права не даёт. Пока адаптер
   пакетного менеджера не выдаёт подтверждённую запись, запуск `Unavailable`.
   Обновления идут через новый каталог версии: замена содержимого активной
   версии запрещена до остановки и подтверждения покоя её работников.
2. **Закрепление.** Supervisor открывает каталог версии и все компоненты
   относительных путей без следования ссылкам; запрещает переход за границу
   каталога. По открытым дескрипторам сверяет тип, владельца, права и запись
   проверенного пакета. Дескриптор исполняемого файла удерживается до `exec`;
   запуск по заново разобранному пути запрещён. Одного `O_PATH`, `stat` или
   хеша без гарантии неизменности активной версии недостаточно. Отказ проверки
   закрывает все дескрипторы и не создаёт работника.
3. **Отдельный помощник запуска.** `carosignd` запрашивает у собственного
   привилегированного однопоточного Linux supervisor запуск подтверждённого
   артефакта, а не исполняет произвольные callbacks после `fork` многопоточного
   процесса. Помощник доступен только службе через закрытый канал, сопоставляет
   запрос с проверенной установкой и своей policy и самостоятельно создаёт
   `socketpair(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0)`. Номер и наличие
   дескриптора задаются существующим договором запуска работника; значение не
   является полномочием. Все остальные дескрипторы закрываются перед `exec`,
   кроме стандартных потоков, перенаправленных в `/dev/null`, управляющего
   конца пары, закреплённого исполняемого объекта до `exec` и закрываемого при
   `exec` канала сообщений об ошибке. Никаких FD устройства или broker на этом
   этапе нет. Родитель закрывает дочерний конец; клиентский конец не публикуется
   по имени и не принимается от работника.
4. **Ограничения до исполнения.** Помощник создаёт отдельный mount namespace с
   приватными распространениями; предоставляет только доступные для чтения
   runtime-файлы, разрешённые policy, и пустое ограниченное пространство для
   временных файлов, без общего `/dev`, пользовательских сокетов и записи в
   дерево установки. Создаёт network namespace без внешних интерфейсов; кроме
   того seccomp deny для создания сетевых и других новых сокетов включён в
   allowlist baseline. Переводит процесс на выделенные непривилегированные
   UID/GID, сбрасывает supplementary groups и все capabilities, устанавливает
   `no_new_privs` и проверенный policy seccomp allowlist до `exec`. Создаёт
   отдельную cgroup v2, задаёт административно утверждённые `cpu.max`,
   `memory.max`, `pids.max` и помещает в неё ребёнка до разрешения исполнения.
   Фильтр и набор runtime-файлов утверждаются установкой для конкретного типа
   runtime; манифест провайдера не может ослабить их самостоятельно.
   Невозможность любого шага означает отказ; молча пропускать namespaces,
   cgroup или seccomp запрещено. Политика открытой сети или дополнительных
   ресурсов требует отдельного согласованного профиля по спецификации.
5. **Exec и наблюдение.** Успех `exec` определяет закрытие при исполнении
   выделенного error pipe; ошибка подготовки/`exec` передаётся по pipe с
   указанием стадии, дочерний процесс завершается и собирается, канал закрыт.
   Выполняется закреплённый объект через `execveat` с `AT_EMPTY_PATH`; окружение
   и аргументы берутся из ограниченного плана, а не из пользовательского
   окружения. Начальный поддерживаемый формат — ELF. Скриптовый runtime и его
   интерпретатор пока получают `Unavailable`, пока не определена безопасная
   передача закреплённого сценария; спецификация позволяет другие языки, и это
   ограничение baseline не объявляет их запрещёнными навсегда. Успешный exec
   ещё не делает работника готовым: действуют сроки Hello 5 с, Initialize 5 с,
   Bind 15 с или утверждённое ограниченное исключение профиля. Точный процесс
   удерживается как дочерний и наблюдается через `pidfd`; PID из сообщения не
   является идентичностью. При ошибке, таймауте или смерти закрываются канал и
   процесс, вызывается предусмотренная договором уборка ресурсов. Неизвестный
   исход команды не повторяется, физический покой не выводится из смерти.
6. **Граница идентичности.** Только supervisor после всех предыдущих проверок
   связывает удерживаемые процесс, канал, `PackageId`, `ProviderId`, `ProfileId`
   и новый `WorkerInstanceId`. Идентификаторы и доказательства не берутся из
   `WorkerHello`. Неподтверждённый канал, в том числе канал тестового процесса,
   нельзя преобразовать в доверенный признаком или конструктором из данных
   работника. Доверенная привязка подтверждает происхождение канала, но не
   аппаратную гарантию, аренду ресурса или успешный фактор.
7. **Минимальный путь для мока.** В испытательном составе существующий отдельный
   `carosign-mock-provider` продолжает запускаться как явно неподтверждённый
   процесс и не выдаёт аппаратных гарантий. Для интеграционного испытания
   supervisor пакет мока и policy устанавливаются в изолированный временный
   образ с теми же проверками, но с явно тестовым корнем доверия; настоящий
   kernel namespace/cgroup/seccomp проверяются внутри подходящей CI-среды.
   Если окружение их не предоставляет, такой интеграционный тест сообщает skip,
   а отдельный тест отказа подтверждает, что production не выполняет fallback.
   Тестовый корень никогда не включается в production-сборку.

#### Порядок реализации и проверки

Сначала определить защищённую запись установленного пакета и владельца policy,
затем supervisor и привязку процесса к каналу, затем исполнение изоляции до
`exec`, затем подключить штатный ProviderManager. Не включать этот путь в
`carosignd` до проверок всех ступеней. Unit-проверки не заменяют системные.

Испытать отсутствие/повреждение policy, подмену или изменение закреплённого
объекта и его родительского пути, ссылку наружу, отказ назначения UID/cgroup,
отсутствие namespace/seccomp, утечку лишнего FD, доступ к сети и `/dev`,
превышение каждого лимита, ошибки каждой стадии и `exec`, недопустимый Hello,
истечение каждого handshake, смерть процесса и PID reuse. Проверить чистку
дочерних процессов при панике/разрыве supervisor и при неуспешной отправке,
запрет повторного mutating dispatch и карантин до подтверждения покоя broker.
Сквозной тест должен связать установленный мок с действительным каналом и
запросом клиента; фикстуры identity, policy и broker назвать явно. Отдельно
проверить обновление при активном работнике: до подтверждённого покоя замена
активной версии запрещена. Испытания не доказывают аппаратной безопасности.
