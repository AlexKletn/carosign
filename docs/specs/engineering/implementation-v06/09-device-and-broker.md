# 09. Device discovery, resource graph and Linux Hardware Broker

## Identification stages

`PlatformInventory` produces ephemeral `DeviceLocator` and hardware signals (ACPI/DT/USB/HID/udev, etc.). `ProviderResolver` compares trusted installed manifest rules to inventory **without** running arbitrary provider code. Multiple approved candidates require explicit system preference or enter `Ambiguous`; do not choose lexicographically. Active probe may only use profile-approved, safe, non-mutating interactions; otherwise require manual binding.

`AuthenticatorId` remains stable only if verified device identity+binding is stable. Raw `/dev/*` path or chip model is not sufficient identity. On ambiguous replacement, refuse automatic association of old registrations.

## Resource graph

Each provider declares `ResourceSet` with access modes (`Exclusive`, `SharedRead`, `Mediated`). Resource Broker policy authorizes package/profile/device binding; manifest declares but does not grant. Scheduler sorts all `ResourceId` bytes lexicographically, requests **entire set atomically**; partial reservation rolls back without any FD transfer. Graph supports shared listeners and exclusive fingerprint controller, but must not incorrectly mark shared QSEE/RPMB resources safe without hardware proof.

### Device states

`Available → Reserved → Active → Quiescing → Available`. On fault: `Uncertain → Recovering → Available | Unavailable`. `WorkerReady` != `DeviceAvailable`. Resource lease `generation` fences stale workers.

## Broker protocol boundaries

Broker implements internal operations `Describe`, `Acquire(resource_set, worker_identity)`, `Delegate(lease, approved_roles)`, `Revoke`, `Release`, `ResetIfSupported`, `GetState`. Each receives platform-validated worker identity, installed policy and exact resource IDs. **No** generic `invoke_any_ioctl` to untrusted provider. Raw QSEE/RPMB handles only to trusted restricted adapter or mediated methods.

### FD rules

- `SCM_RIGHTS` allowed only in Provider `BindAuthenticator`; `fd_slots` explicitly enumerate roles (device, event, sealed config, mediated RPC) and descriptor indices 0..N-1.
- Validate exact descriptor count; close all on mismatch, unknown role, unexpected type. `O_CLOEXEC` on inherited copies except intentionally passed socket; no descriptor leaks.
- A passed FD remains accessible even after broker closes its own copy. **Never claim logical revocation shuts down raw FD use**. If unable to revoke hardware rights, terminate isolated worker and use driver/reset evidence to determine whether device is safe.
- Worker does not get host file paths to arbitrary storage/RPMB. Where needed give scoped service or directory FD with minimal access.

## Linux process sandbox baseline

Dedicated unprivileged UID per approved provider class, no network by default, own mount namespace and only specified device/storage FDs, cgroup CPU/memory/pids limits, seccomp allowlist matched to runtime needs, no ambient capabilities, privilege escalation disabled. For Python/C++/GLib providers relax only specific sandbox calls with review; **never** grant full root because language runtime needs it.

Verified launch: package metadata under root-owned trusted directory, executable opened/validated and bound to spawn without path race where supported; process tracked by pidfd; worker communicates only over inherited private socket. `WorkerHello` self-reported ID does not authenticate package.

## Under-display optical interaction

`InteractionController` owns display illumination + touch capture lease separate from provider worker. Start only after trusted platform has established allowed region and current interaction generation. On cancellation or revoke: release illumination/contact lease, verify quiesce; failure → `Uncertain` resource. Linux may provide untrusted UX path; Veles integrates through Явь. Neither can claim Trusted UI without independently proved barriers.

## Faults

Broker crash: all outstanding privileged leases become Uncertain unless kernel/device independently enforces closure. Provider crash during active verification: suppress outcome + recovery. Provider crash after mutation arm: block enrollment scope + reconciliation. Hot unplug invalidates epoch and owner leases immediately. Suspend/resume requires profile-defined device resume flow; old operations expire, previous proof not reusable.

### Required `ResourceId` binding and FD role resolution (v0.6.1)

Each `FdSlot.resource_id` is the nonzero, 16-byte ID of a specific resource listed in the **currently active** broker-issued lease. The binding tuple is `(lease_id, worker_instance_id, authenticator_id, device_epoch, resource_id, role, handed_fd_identity)`. The broker records this tuple before delivery, including approved handle type and device identity. Provider Manager must verify it atomically with transfer; worker validates `FdSlot` indices/roles and handle shape. `BindAuthenticator` must carry every FD, including broker-mediated socket FDs. No other `carosign.provider.v1` packet carries descriptors. Unknown role/resource, duplicate index, unapproved duplicate resource or descriptor mismatch fails binding, closes all received FDs, and never activates the sensor.

Role types: DEVICE=approved device handle identified by broker (do **not** infer authorization from char-device type alone); EVENT=approved Linux eventfd; CONFIG=sealed read-only memfd; MEDIATED_SERVICE=connected Unix `SOCK_SEQPACKET` service channel to broker implementing `carosign.broker.v1`. A mediated service is a **revocable RPC authorization**, not a raw QSEE/RPMB FD; every request is checked against current lease and trusted allowlisted handler. `resource_id` is part of the lease and NOT the same as PackageId/ProfileId. ResourcePort provides internal broker-issued claims including these binding tuples; its Rust `acquire` return must be linked to `Delegate(lease, roles)` before Bind. A worker-supplied `resource_id` cannot create authorization. No approved handler for an operation => `UNSUPPORTED`, do not forward arbitrary bytes to a device.

See [25](25-v061-conflict-resolutions.md) and [`carosign.broker.v1`](../../contracts/proto/carosign/broker/v1/broker.proto).

For mediated calls that may modify persistent TA state, broker reads a trusted `DispatchAuthorizationPort` binding `operation_id+mutation_id+command_id` to durable `DispatchArmed` and current worker/lease/epoch before invoking the handler; worker-supplied IDs do not prove this. Unknown post-dispatch outcome remains `NeedsReconciliation`. See [26](26-mediated-broker-v1.md).

### v0.6.5: broker bulk bodies and credential sink

Delegating `FD_ROLE_MEDIATED_SERVICE` grants access only to already-approved typed handlers. On **that broker socket only**, version 1.1 admits one sealed memfd with `Call`/`CallResult.external_body`; it never changes `BindAuthenticator`-only `SCM_RIGHTS` on provider control. A trusted broker must validate exact FD/worker/lease/resource/epoch/handler identity and body schema before staging into kernel/DMA, and retain hardware memory until safe quiescence. HAT references are resolved and consumed within a separately approved credential-authority sink bound to an armed mutation; a provider receives no HAT bytes or arbitrary access to Gatekeeper. See [31](31-v065-enrollment-authorization.md), [32](32-v065-bulk-command-buffer.md).

## Владение выдачей, ограждение и посредник

<a id="resource-lifecycle"></a>

### Владение ресурсами и подтверждённая уборка

Доверенный посредник атомарно выдаёт неклонируемый `ResourceLease` для точного `ResourceSet` и владельца (операция, работник, аутентификатор, эпоха). Одноразовый комплект FD для Bind проверяет каждый ресурс, роль и идентичность дескриптора. Владелец операции удерживает выдачу до завершения. `quiesce` подтверждает покой той же области, а отдельный `release` потребляет выдачу и подтверждение. Смерть процесса, ответ работника или `Drop` не подтверждают уборку.

#### Точный договор

1. Внутренний владелец выдачи — типизированная связка `OperationId`,
   `WorkerInstanceId`, `AuthenticatorId`, `DeviceEpoch`. Посредник получает
   её из проверенного состава платформы, не из ответа работника. `ResourceId`
   и упорядоченный `ResourceSet` остаются отдельными идентификаторами.
2. `ResourceLease` выдаётся только доверенным адаптером посредника, имеет
   закрытые поля, не реализует `Clone` и несёт точную связку владельца,
   эпохи и всего набора. Повторная или частичная выдача — отказ; номер
   `LeaseId` назначает посредник. Захват набора остаётся атомарным.
3. `delegate` принимает активную выдачу и разрешённые политикой роли.
   Возвращаемый пакет дескрипторов однократный; проверяются конкретные
   `(lease, worker, authenticator, epoch, resource, role, fd identity)`.
   Только он может быть передан в `BindAuthenticator` через `SCM_RIGHTS`.
   Закрытие копии FD не считается отзывом дескриптора у работника.
4. Разделить `quiesce` и `release`. `quiesce` запрашивает подтверждение у
   доверенного посредника/аппаратного адаптера и возвращает либо закрытый
   внутренний результат для **той же** выдачи, либо типизированную
   неопределённость. Ответ провайдера и смерть процесса сами по себе не
   доказывают покой. Если у устройства нет способа подтвердить покой,
   результат остаётся `Uncertain`.
5. `release` потребляет выдачу и подтверждение, повторно сверяет идентичность,
   текущую эпоху и точный набор. Только после успешного освобождения
   подтверждается `CleanupConfirmed`. Устаревшая выдача, повтор, обрыв,
   неоднозначный результат или недоступный посредник не освобождают новую
   выдачу и ведут к карантину; потребитель не получает ложный успех.
6. Владелец операции удерживает выдачу и актор через ожидание провайдера,
   отзыв и завершение. Асинхронные вызовы не выполняются под общими замками.
   `Drop` может инициировать уборку, но не считается её подтверждением;
   неопределённый ресурс сохраняется под контролем координатора.
7. Отсутствующий посредник возвращает `Unavailable`; тестовый посредник
   остаётся в тестовом коде и не выдаёт аппаратную гарантию. Конкретные
   аппаратные команды Goodix/TA сюда не добавляются.

Это решение изменяет **внутренние** Rust-порты и типы владения. Существующие
Protobuf-поля и их значения не меняются; если для отдельного управляющего
канала брокера понадобится новый wire-договор, он согласуется отдельно.

#### Проверка после принятия

Проверить точный набор и владельца, чужую и старую эпоху, повтор выдачи и
освобождения, частичный и повреждённый ответ посредника, переполнение и
ошибку передачи FD, смерть работника до и после отправки, отзыв при ожидании,
зависшую/неподтверждённую уборку, изменение эпохи во время уборки и отсутствие
ложного `CleanupConfirmed`. Отдельно проверить, что после неопределённости
конкурентная операция не получает ресурс. Настоящее устройство и его
физический покой подтверждаются только устройственно-специфичными проверками.

<a id="resource-quiescing"></a>

### Ограждение точной выдачи во время уборки

До внешнего ожидания планировщик под коротким замком сверяет всю активную выдачу, включая режимы, переводит её в `Quiescing` и создаёт одноразового держателя уборки. Новые выдачи затронутых ресурсов блокируются, включая совместное чтение. Другие уже выданные наборы сохраняются. Замок не удерживается через ожидание. Только `Released` от доверенного посредника для удерживаемой области и текущей эпохи снимает её ограждение. `Quiesced` удостоверяет только покой. Неопределённый ответ сохраняет блокировку до восстановления; повтор по утрате ответа запрещён.

#### Точный договор

1. Планировщик и посредник используют одну упорядоченную неизменяемую область:
   `LeaseId`, `ResourceOwner` (операция, работник, аутентификатор и эпоха) и
   канонический `ResourceSet` с режимами. Доверенный состав назначает область;
   данные работника, сам номер операции или совпадение полей не являются
   полномочием. Активный владелец хранится при атомарной выдаче набора.
2. До внешнего ожидания единый владелец под коротким замком планировщика
   проверяет совпадение **всей** активной области и переводит её из `Active`
   в `Quiescing`. С этого момента новые выдачи любого затронутого ресурса,
   включая совместное чтение, запрещены. Одновременно захватывается
   неповторяемый внутренний держатель уборки. Чужая область, повтор и старая
   эпоха отказывают без изменения планировщика. Другие уже выданные читатели
   остаются владельцами своих выдач; их нельзя освобождать этим переходом.
3. Замки таблицы операций и планировщика не удерживаются во время `await`.
   Порядок захвата замков в реализации должен быть единым; переходы между
   владельцами фиксируются до освобождения соответствующего замка. Отзыв и
   истечение срока не снимают `Quiescing` и подавляют результат операции.
4. Неподтверждённый вызов `quiesce` или `release`, потеря держателя, timeout,
   ошибка, старый ответ либо прекращение процесса переводят ограждение в
   `Uncertain` или сохраняют его; ни повторная отправка, ни `CleanupConfirmed`
   не допускаются. Нельзя повторно запустить уже начатый внешним посредником
   этап только из-за потери ответа. Перезапуск процесса требует отдельного
   устойчивого восстановления/карантина: локальная память его не доказывает.
5. `Quiesced` удостоверяет только покой в той же полной области и выдаётся
   после проверки ответа **доверенного посредника**. `Released` — отдельное
   неповторяемое подтверждение успешного освобождения той же области. Его
   создание ограничено проверенным адаптером посредника; публичный
   конструктор из `LeaseId`/полей или ответ провайдера недопустимы.
   `Quiesced` не снимает блокировку и не равен `Released`.
6. Только `Released` для удерживаемого `Quiescing` и текущей эпохи позволяет
   атомарно удалить именно эту выдачу, снять её ограждение и рассмотреть
   очередь. Старый `release(LeaseId)` не принимает и не обходит ограждённую
   выдачу. Если подтверждение/контекст не совпадает, выдача остаётся закрытой.
   Проверка/повтор номера не освобождает соседнего владельца `SharedRead`.
7. Планировщик не устанавливает физический покой, доверие к FD или
   происхождение полномочий. Если посредник не может подтвердить покой или
   освобождение, соответствующее устройство остаётся недоступным для нового
   использования. Аппаратные команды и реализация провайдера не добавляются.

#### Требуемые изменения

- Привести `Scheduler` и его запрос выдачи к полному `ResourceOwner` и
  `ResourceSet`, ввести закрытый одноразовый держатель `Quiescing` и отдельный
  переход на проверенное освобождение. Согласованно обновить все реализации
  и потребители; сохранить FIFO и предел очереди спецификации.
- Подключить этот переход к `cleanup_verification` и `EffectOwner` до первого
  внешнего ожидания; удерживать результаты и шаг актора до передачи владельцу.
- Обеспечить, чтобы данные и FD связывались с действующей выдачей доверенным
  брокером; тестовый источник не становится production-подтверждением.
- Внешние Protobuf и wire-роли не менять этим ADR.

#### Проверки после принятия

На общем планировщике и с управляемым ожиданием посредника проверить:
конкурентный `SharedRead` до ответа и после неопределённого исхода; чужие
работник, устройство, эпоха, lease, ресурс и режим; повтор и потерю
держателя; отказ quiesce/release, timeout, отмену и перезапуск; отдельные
выдачи двух совместных читателей; освобождение после точного подтверждения;
старое подтверждение против нового владельца. Ни один тест с подставным
посредником не считается доказательством физического покоя.

<a id="broker-single-sender"></a>

### Релейный канал в испытательном составе

В релейной схеме только посредник владеет стороной платформы провайдерского канала, назначает исходящий `frame_seq` и выполняет Bind с проверенными ресурсами. Демон общается по отдельному каналу без ancillary FD и независимо проверяет контекст наблюдений. ADR принят для реализации и испытаний. Применимый к первому выпуску порядок системного допуска и передачи команд определяется документом 35; принятие релейной схемы само по себе не открывает production Acquire/Bind.

#### Точный договор

1. Системный supervisor создаёт private provider `SOCK_SEQPACKET` pair и
   отдельный private tunnel pair **до exec обоих процессов**. Конец provider
   выдаётся worker при запуске как единственный разрешённый FD управления;
   второй конец и один конец tunnel — по наследованию закреплённому процессу
   посредника. Второй конец tunnel остаётся у демона. Лишние копии закрываются
   до запуска обработки; worker не получает ни ресурс до Bind, ни tunnel.
   Успешное `exec`, происхождение обоих экземпляров, их права и связь каналов
   независимо устанавливает доверенный supervisor. Унаследованный FD, UID,
   `SO_PEERCRED`, Hello и экземпляр сами по себе не полномочия.
2. У каждого worker ровно один посредник-читатель и один посредник-писатель
   provider-канала. Только посредник назначает исходящий `frame_seq`; демон
   не сохраняет копию provider-FD, не пишет Bind непосредственно и не
   выбирает номера кадров. При назначении последовательности данные из tunnel
   проходят полный разбор и проверку направлений; допускаются только
   утверждённые команды `carosign.provider.v1` и связанный контекст. Входящие
   последовательности проверяются посредником, а демон независимо проверяет
   worker, epoch, operation, command, область и порядок событий.
3. Единственный `BindAuthenticator` формируется посредником после независимой
   проверки политики, установленного профиля, живой выдачи, worker и эпохи.
   Ресурсы открываются **самим посредником** или поступают по уже отдельно
   утверждённой границе; точные `(resource, role, mode, FD identity)`
   закрепляются до отправки. `SCM_RIGHTS` допускается только в этом Bind на
   provider-канале. Tunnel и control никогда не несут ancillary FD; лишние FD
   закрываются, соединение и затронутая операция переходят в отказ.
4. Hello → Initialize → Bind → Ready сохраняют версии и semantics документа
   06, включая provider minor >= 3. Отказ policy, неверный контекст, отзыв,
   неподтверждённый профиль или исчерпание очереди даёт отказ до действия.
   Время, номер, путь, маркер БД или сообщение worker не доказывают права.

#### Форма внутреннего tunnel

До принятия точной общей схемы `adr-broker-control-wire-proposed.md` tunnel
не включать в production. Для реализации и испытаний принята самостоятельная
схема `carosign.broker_relay.v1`, без изменения provider или client wire:

```proto
syntax = "proto3";
package carosign.broker_relay.v1;
import "carosign/provider/v1/provider.proto";
import "carosign/common/v1/common.proto";
message Frame {
  uint32 major = 1;           // 1
  uint32 minor = 2;           // 0
  uint64 seq = 3;             // 1.., отдельно по каждому направлению
  bytes worker_instance = 4;  // 16 ненулевых байт
  uint64 device_epoch = 5;    // ненулевая
  oneof body {
    Hello hello = 10;
    Command command = 11;
    Event event = 12;
    Bind bind = 13;
    BindResult bind_result = 14;
    Control control = 15;
    ControlResult control_result = 16;
    CommandResult command_result = 17;
  }
}
message Hello { bytes broker_instance = 1; bytes boot_id = 2; }
message Owner {
  bytes operation_id = 1; bytes worker_instance = 2;
  bytes authenticator_id = 3; uint64 device_epoch = 4;
}
enum AccessMode { ACCESS_UNSPECIFIED = 0; EXCLUSIVE = 1; SHARED_READ = 2; MEDIATED = 3; }
message Claim { bytes resource_id = 1; AccessMode mode = 2; }
message Scope { bytes lease_id = 1; Owner owner = 2; repeated Claim claims = 3; }
message Slot { bytes resource_id = 1; carosign.provider.v1.FdRole role = 2; }
message Command { bytes request_id = 1; carosign.provider.v1.ProviderFrame frame = 2; }
message CommandResult {
  bytes request_id = 1;
  oneof result { bool sent = 2; carosign.common.v1.Error refused = 3; }
}
message Event { carosign.provider.v1.ProviderFrame frame = 1; }
message Bind {
  bytes request_id = 1; Scope scope = 2; repeated Slot slots = 3;
  carosign.provider.v1.ProviderFrame frame = 4;
}
message BindResult {
  bytes request_id = 1;
  oneof result { bool sent = 2; carosign.common.v1.Error refused = 3; }
}
message Cancel { Scope scope = 1; bytes cancel_command_id = 2; }
message Revoke { Scope scope = 1; }
message Control {
  bytes request_id = 1;
  oneof action { Cancel cancel = 2; Revoke revoke = 3; }
}
message ControlResult {
  bytes request_id = 1;
  oneof result {
    bool cancel_sent = 2; bool revoked = 3; carosign.common.v1.Error refused = 4;
  }
}
```

`Owner` содержит ненулевые ID и эпоху, `claims` — от 1 до 8 уникальных
ресурсов по возрастанию ID, `mode` и `role` — известные ненулевые значения.
`Bind.frame` обязан содержать только `BindAuthenticator`; его slots сверяются
с `slots` и действующей выдачей. `Command.frame` допускает только команды,
разрешённые в текущей фазе и профиле, кроме Bind; направление исходного
ProviderFrame — daemon → worker. Посредник отвергает ненулевой `frame_seq`
в запросе и назначает его сам. `Event.frame` допускает только worker-события;
направление и последовательность проверяются до пересылки. `sent=true` —
только подтверждение одного полного локального send, не ACK worker, не
аппаратный покой и не право. `sent=false`, неизвестный result, неизвестная
ошибка или поля success вместе с error запрещены. `ControlResult` для Cancel
принимает только `cancel_sent=true`, для Revoke — только `revoked=true`;
эти подтверждения не взаимозаменяемы и ни одно не подтверждает покой.
`request_id` уникален в
жизни tunnel и не является полномочием. `Cancel` посылает существующую
типизированную команду отмены с новым ненулевым `cancel_command_id` и
`target_operation_id` из проверенного `scope.owner`; повтор ID запрещён.
`Revoke` запрещает
последующие отправки в scope и не подтверждает покой. Неопределённый send
закрывает tunnel и не кодируется как `refused`. Предлагаемый закрытый набор
команд и поведение при отказах зафиксированы ниже.
В договоре нет `invoke_any` или безусловной пересылки любого protobuf.
Один tunnel seqpacket ограничен 65536 байт; если внешняя оболочка уменьшает
доступный размер provider-кадра, нужен явный отказ либо отдельный проверенный
фрагментированный договор, а не усечение. Все ID — 16 ненулевых байт;
неизвестный enum/поле, повтор singular/oneof, неверное направление, вложенный
Bind вне выделенного сообщения, FD или нарушение seq закрывают tunnel.

Один исходящий запрос с данным `request_id` имеет один ответ; повтор не
исполняется, исход не выводится из отсутствия ответа. В этом предложении
`request_id` получается XOR полного 16-байтового `broker_instance` и числа
`seq`, расширенного до 16 байт в big-endian. Это взаимно однозначное
преобразование в пределах одного экземпляра; нулевой результат запрещён и
закрывает tunnel. Брокер сверяет ID с кадром, ответ копирует его.
Неповторяемый broker_instance выдаёт системный состав; ошибка выдачи или
переполнение счётчика закрывает tunnel. Это ограничивает
учёт повторов без неограниченного множества ID и не создаёт авторизацию.
События независимы от
ответа и упорядочены с ним одним читателем. Для отмены во время блокирующей
работы требуется доказанный неблокирующий исполнитель: чтение tunnel и
обработка управления не ждут завершения hardware-команды. Ограничения и
приоритеты очередей предложены ниже. Переполнение — безопасное закрытие, а не
отбрасывание терминального события; работоспособность при блокирующем вызове
требует отдельного воспроизводимого испытания.

#### Обязательные правила исполнения

Эти принятые для реализации и испытаний правила не создают источника доверия
и не разрешают запуск production.

##### Закрытый перечень сообщений

Направление daemon → broker допускает ровно: `Hello`, `Bind`, `Control`,
а в `Command.frame` — только `Initialize`, `Describe`, `StartVerification`,
`PrepareEnrollmentChallenge`, `StartEnrollment`, `StartRemoval`,
`QueryMutation`, `Quiesce`, `CloseAuthenticator` и `Shutdown` из provider
v1.3. `RequestCancel` допускается только как `Control.Cancel` и формируется
самим посредником; второй способ передачи отмены запрещён. `BindAuthenticator`
допускается исключительно как `Bind.frame`. Все прочие payload, в том числе
события worker и `WorkerHello`, в `Command.frame` запрещены. Наличие имени в
перечне не разрешает команду: фаза, профиль и полномочия проверяются отдельно.

Направление broker → daemon допускает `Hello`, `CommandResult`, `BindResult`, `ControlResult`
и `Event`. В `Event.frame` допустимы только `WorkerHello`, `Initialized`,
`AuthenticatorReady`, `Described`, `CommandAck`, `ProviderProgress`,
`VerificationOutcome`, `MutationOutcome`, `QueryMutationResult`,
`CancelReceipt`, `EnrollmentChallengeReady`, `Quiesced` и `ProviderFault`.
Порядок provider-handshake и семантика каждого кадра остаются по документу 06;
посредник и daemon независимо проверяют контекст и порядок. Один обычный
терминальный исход относится только к принятой команде; дубликат закрывает
сеанс. В частности, `ProviderFault`, `Quiesced` и `CancelReceipt` не означают
успех операции или освобождение устройства.

`CommandResult.sent=true` подтверждает только один полный локальный
provider-send, а `refused` допустим лишь при доказанной неотправке. Приём
tunnel-пакета не подтверждает provider-send. ACK работника и его исход — только
проверенные `Event`; обрыв после начала записи provider-кадра означает
неопределённый исход и не кодируется ни одним из двух вариантов. Неизвестный
`request_id`, `sent=false`, незаданный `oneof` или повтор ответа закрывают
сеанс. После `refused` нельзя оставлять сообщение на дальнейшую отправку.

##### Пределы и отмена

Одна запись tunnel и одна запись provider ограничены 65536 байтами каждая,
с точными границами документа 13. Проверяется **сериализованная внешняя**
оболочка; кадр, который помещается в provider, но не помещается в tunnel из-за
оболочки, отклоняется до provider-send как неотправленный. Фрагментации,
усечения и скрытого снижения заявленных возможностей нет; несовместимый
профиль остаётся недоступным. `Bind` допускает 0..8 FD и ровно столько же
согласованных слотов, tunnel/control — строго ноль FD. После разбора любой
неожиданный FD немедленно закрывается вместе со всеми FD пакета.

Принят максимум 64 ожидающих обычных запросов, 8 ожидающих
управляющих запросов, 64 исходящих событий и по 8 MiB сериализованных данных
на каждое направление tunnel для одного worker. Это локальные
пределы; конфигурация может только
уменьшать их. Состав должен дополнительно ограничивать суммарную память и
число процессов; происхождение этого общего лимита остаётся открытым. Резерв
очереди управления нельзя занимать обычными командами или прогрессом.

Единственный владелец provider-FD выполняет неблокирующие отправки по одной;
перед началом каждой проверяет принятый отзыв. Отмена и отзыв обслуживаются
перед следующим обычным запросом; начатый syscall не прерывается. Реализация
не держит единственный поток чтения управления на синхронном вызове hardware.
`EAGAIN` до записи хранит один незавершённый запрос до его срока и не считается
ACK; новый запрос с тем же ID не выполняется. Иной неоднозначный исход или
истечение после возможной передачи ведёт в `Uncertain`, блокирует выдачу и не
повторяет команду. Если очередь событий не принимает результат или терминал,
посредник прекращает сеанс и переводит затронутую область в неопределённость;
он не отбрасывает событие и не сообщает успех. На заполнение обычной очереди
до side effect возвращается `BUSY`; на заполнение управляющей очереди дальнейшие
обычные отправки запрещаются и сеанс закрывается с карантином.

##### Доверие и восстановление: минимальный fail-closed состав

Системный состав предоставляет посреднику **два независимых доверенных
источника**: утверждённые установку/профиль с перечнем ресурсов и ролей и
проверяемые решения политики, привязанные к текущей операции, worker, эпохе,
цели и сроку. Каждый запрос проверяется у источников перед действием, а после
любого внешнего ожидания проверка и сверка всей области повторяются. Срок
берётся из проверенного решения политики, привязанного к операции, а не из
непроверенного сообщения daemon. Ссылка
`request_id`, lease, запись metadata и сведения worker не заменяют источники.
Как эти источники создаются, защищаются от подмены/отката и отзываются между
процессами, в данном предложении **не определено**: до отдельного решения и
проверки настоящий `Acquire`/`Bind` недоступен. Test-only заглушка не
экспортируется в production и не выдаёт аппаратных гарантий.

Посредник сериализует проверенные решения `Revoke` и начало записи по одной
области: подтверждённый отзыв, принятый до перехода `Sending`, запрещает send;
если send уже начат, результат остаётся неопределённым до надёжного
подтверждения. Отзыв не отзывает raw FD у worker. При потере daemon, broker,
worker или источника отзыва результат подавляется, затронутая область и
ресурсы остаются `Uncertain`, а armed-мутация — `NeedsReconciliation`.
Источник отзыва должен непосредственно доставлять его посреднику и позволять
сериализовать проверку актуальности с переходом `Sending`. Подтверждение
`revoked=true` выдаётся только после установки запрета у единственного
отправителя; постановка отзыва в очередь daemon таким подтверждением не
является. Если источник не обеспечивает эту границу, отправка запрещена.

Для начального fail-closed восстановления предлагается правило: **каждый**
новый экземпляр broker после запуска отказывает во всех выдачах до получения
от независимого профильного восстановителя доказательства безопасного
состояния каждого запрашиваемого полного набора и текущей эпохи. Пустая
оперативная таблица или отсутствие журнала не являются таким доказательством.
Восстановитель должен также учитывать запись `DispatchArmed` в одной
транзакционной области metadata и не давать повтор мутации; при ошибке
чтения, неизвестном commit или невозможности установить точную область
закрывается весь затронутый профиль. Этот консервативный порядок допускает
недоступность после чистого запуска, пока независимого проверяющего нет. Он
**не** определяет доказательство физического покоя, источник anti-rollback,
схему устойчивого владения полномочиями или протокол восстановителя; до их
отдельного решения и процессных испытаний production-выдача закрыта.

#### Отзыв, ошибки и восстановление

Посредник — единственный сериализующий владелец отправки Bind и проверенного
сигнала отзыва. Поимённые стадии: `Prepared → Sending → Sent`, с переходами
`Prepared → Revoked` и `Sending → Uncertain`. Принятый отзыв до начала send
запрещает Bind. После возможной передачи, потери подтверждения или смерти
отправителя повтор Bind запрещён; выдача и весь набор ресурсов ограждаются.
Одного межпроцессного файлового lock недостаточно: ни его освобождение, ни
смерть владельца не доказывают отсутствия посланного кадра.

Доверенный системный источник обязан сохранять неопределённую область при
потере **любого** из демона, посредника или worker и восстанавливать её после
перезапуска; пустой новый процесс не подтверждает свободные ресурсы. Физический
покой и `Released` подтверждает независимый профильный адаптер, не relay, не
закрытие FD и не worker ACK. Пока этот устойчивый источник и его атомарность
не определены и не реализованы, production Acquire после сбоя запрещён.
Проверка оборудования и устройственно-специфичный адаптер — отдельные проекты.

#### Испытания до принятия реализации

- Реальные отдельные процессы daemon/broker/worker: один `SCM_RIGHTS` на Bind,
  ноль FD в tunnel/control, фактическая работоспособность выданного сокета и
  запрет доступа worker до Bind.
- Одновременные события и команды, отмена при блокирующем вызове, исчерпание
  очередей, лишние/переставленные/дублированные кадры и обрыв на каждой стадии;
  отсутствие двойной отправки и потери терминального события.
- Смерть каждой стороны до и после каждого действия, перезапуск с сохранённым
  карантином и независимая проверка уборки. Тестовый брокер не доказывает
  аппаратный покой. Отдельно проверить ограничения UID, namespace, seccomp и
  исходящие соединения в целевом составе.
- Отдельная независимая реализация кодека и межъязыковая проверка wire; явное
  сопоставление с документами 06/09/20/26/32 и принятым supervisor ADR.
