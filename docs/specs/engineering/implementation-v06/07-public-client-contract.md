# 07. `carosign.client.v1`: client-facing service and PAM mapping

Семантика расширенного описания аутентификатора и событий захвата определена
в [документе 36](36-authenticator-types-and-events.md).

**Authoritative schema:** `contracts/proto/carosign/client/v1/client.proto` + common. Linux default: public `AF_UNIX/SOCK_SEQPACKET` endpoint `/run/carosignd/control.sock`. Transport access does **not** authorize operations; `SO_PEERCRED` establishes OS peer identity only, and trusted authority checks operation scope separately.

## API surface (v1)

| Request | Who may call | Main response | Notes |
|---|---|---|---|
| `Hello` | any local peer allowed to connect | `HelloAck` | major=1; agree minor, features |
| `ListAuthenticators` | policy-approved observer | descriptors, optional provider inventory | no private enrollment data |
| `GetAuthenticator` | policy-approved observer | capabilities/health | declared features, not trust proof |
| `ListEnrollments` | subject owner / delegated admin | bounded labels and refs | no template bytes |
| `BeginVerification` | auth consumer authorized for purpose/subject | `OperationAccepted` | `idempotency_key` scoped |
| `BeginEnrollment` | manage-factor grant required | `OperationAccepted` | profile/subject and CAS |
| `BeginRemoval` | manage-factor grant required | `OperationAccepted` | expected generation mandatory |
| `ObserveOperation` | authorized owner/observer | events/snapshot/gap | ID not bearer permission |
| `CancelOperation` | authorized owner/system policy | immediate receipt | receipt != terminal cancellation |
| `GetOperation` | authorized observer | snapshot | bounded and privacy filtered |

Not in v1: public raw images, template export, arbitrary TA commands, `Identify` without claim, secret/key retrieval, remote TCP API, generic device ioctl.

## Verification API semantics

- Client requests `subject_selector`, `authenticator_selector`, `purpose_hint` and opaque `authority_request_ref`. Only trusted authority selects definitive SubjectRef, purpose and grant. If intent conflicts with policy, reject.
- Runtime generates `OperationId`, `BootDeadline`, fresh context and binds to owner. `OperationAccepted` ≠ match.
- Progress: `PlaceFinger`, `MoveFinger`, `RemoveFinger`, `Retry`, `Working`. Standard text is localized **client-side** from event enum; any provider-provided free text treated diagnostic/untrusted, not UI directive.
- External terminal outcome `AcceptedFactor` delivered **only after required evidence acceptance and grant revalidation**, otherwise `NotMatched`, `NoSample`, `Cancelled`, `Unavailable`, `Failed`, `Indeterminate` as appropriate. `AcceptedFactor` means a factor accepted under *that* request, **not** session authorized.
- Evidence token bytes never returned to public client. Trusted authority/KeyMint integration uses a separate restricted port; `EvidenceRef` cannot be replayed for another purpose.

## Idempotency

`idempotency_key`: exactly 16 bytes, nonzero, scope `(authenticated caller binding, subject, operation kind, canonical parameters)`. Runtime stores canonical digest and selected op. Within the operation’s retention window, the same key and digest identify the existing operation. Reusing a retained key with a different digest returns `CONFLICT`. A different key does **not** justify retrying mutating hardware action while same scope is pending. After reboot, verification requests expire; incomplete mutations are discoverable via persisted reference under authority permission.

## Operation ownership and observing

`OwnerHandle` bound to the original authenticated client and grant watcher; `ObserverHandle` separately authorized. `ObserveOperation(after_sequence)` replays existing events within bounded ring; missing history yields explicit `Gap` followed by snapshot. Terminal mutation outcome lives durably and may be acknowledged only by authorized owner; progress is not durable contract. No automatic cancellation on observer disconnect.

For verification, owner death/revocation triggers suppression of success and best-effort cancellation. For mutation, owner death stops new steps, but pending hardware state may require reconciliation and persists independent of client.

## PAM adapter rules

PAM adapter is *client only*, no sensor access. It receives `PAM_USER` and trusted service context, but backend must verify authorization for that subject. Outcomes mapping:

**Принятое изменение B-04 для Linux:** демон создаёт новый неповторённый
`TransactionId` для каждого вызова PAM, после установления контекста
соединения, и связывает его с конкретным соединением и вызывающим. Для этого
профиля PAM использует отдельное соединение на один вызов и закрывает его при
завершении. Идентификатор — только коррелятор, не полномочие и не доказательство
личности или цели. Подтверждение клиента и экземпляра службы, разрешение
субъекта и цели политикой, привязка к конкретному вызову, отзыв и повторная
проверка остаются отдельными обязательными условиями успешного результата.
Никакой специальной интеграции `sudo` или другого отдельного приложения не
требуется. При недоступности обязательного подтверждения положительный исход
закрыт. Выбор конкретных системных механизмов этим изменением не утверждён.

| Accepted factor | `PAM_SUCCESS` only when bound to exact PAM request and policy accepts |
|---|---|
| `NotMatched` | `PAM_AUTH_ERR` |
| `Cancelled`, `Unavailable`, daemon error | `PAM_AUTHINFO_UNAVAIL`, allow configured password fallback |
| Invalid/unknown subject | `PAM_USER_UNKNOWN` |
| `Indeterminate`/invalid evidence | `PAM_AUTHINFO_UNAVAIL`, never success |

Do not ship automatic edits to `/etc/pam.d` during installation; provide example stacks and testing procedure with root recovery path. Keyring secret release is **not** part of `carosign.client.v1`; it belongs to separate trusted secret consumer, never implicit `PAM_AUTHTOK` propagation by core.

## Endpoint/resource limits

One frame max 64 KiB. Public peer must complete hello within 5 seconds. Default 4 active/queued operations per connection; max 16 queued per authenticator; default max 8 observers per operation; event ring 128. Implementation must support admin policy for tighter limits and fail with typed `LIMIT_EXCEEDED`, never unbounded queue allocation.

## Контекст запроса и системные клиенты

<a id="client-verification-context"></a>

### Сохранение контекста клиентской проверки

`BeginVerification` сохраняет проверенные wire-поля `subject`, `purpose_hint` и `authority_request_ref` при переводе в доменный запрос. Селектор, подсказка цели и непрозрачная ссылка передаются вместе с доверенным `CallerContext` в IdentityAuthority и PolicyAuthority. Субъекта, цель и полномочие устанавливают эти владельцы. Диагностика скрывает содержимое полей. При недоступности обязательной доверенной границы ответ — `Unavailable`.

#### Точный договор

1. Дополнить существующий `ClientRequest::BeginVerification` полями
   `subject: common::SubjectSelector`, `purpose_hint: common::VerificationPurpose`,
   `authority_request_ref: Vec<u8>`; имеющиеся `idempotency_key: Vec<u8>` и
   `authenticator: Vec<u8>` сохранить без переименования. `subject` получается
   из обязательного, уже проверенного protobuf-поля и содержит ровно один из
   `current_subject`, `account_hint`, `opaque_authority_selector`;
   `purpose_hint` преобразуется в известное перечисление после валидации.
   Дополнить `DomainRequest::BeginVerification` теми же тремя полями, оставив
   существующие типизированные `idempotency` и `authenticator`; `to_domain`
   переносит их без смысловых преобразований. `Debug` обоих запросов остаётся
   скрывающим содержимое. Wire, номера и смысл полей не меняются.
2. Передавать эти значения вместе с `CallerContext`, полученным доверенным
   переходником соединения, в `IdentityAuthority` и `PolicyAuthority`.
   Селектор, подсказка цели и непрозрачная ссылка — **недоверенные данные**,
   не субъект, не цель и не grant. Только authority разрешает субъект,
   устанавливает цель и выдаёт проверенное полномочие. UID/PID/GID сами по
   себе полномочия не создают; `Asked` не используется как замена authority.
3. До создания операции применять идемпотентность в области,
   установленной документом 07: привязанный caller, разрешённый субъект,
   вид операции и канонические параметры. Совпадающий ключ и digest возвращают
   существующую операцию, несовпадение digest — `CONFLICT`. Данные действуют в
   пределах текущего запуска; это не означает автоматический повтор команды.
4. Не включать успешную публичную проверку до подключения доверенных
   identity/policy адаптеров, разрешения регистраций, контроля срока и отзыва,
   проверенного провайдера и безопасной доставки. При их отсутствии сохранять
   отказ `Unavailable`.

#### Проверка после принятия

Покрыть сохранение каждого поля и каждого варианта селектора, подмену субъекта,
цели и ссылки, повтор и конфликт ключа для разных caller и субъектов, отказы
authority, отзыв и истечение срока. Сквозные случаи выполнять через настоящий `SOCK_SEQPACKET`,
проверяя отсутствие вызова провайдера при отказе.

<a id="verification-idempotency"></a>

### Атомарная идемпотентность клиентской проверки

Владелец операций атомарно проверяет ключ, выделяет место и публикует запись и актор. Ключ записи — `(caller_binding, idempotency_key)`, где привязку транзакции устанавливает доверенный адаптер. После свежей проверки identity и policy совпадающий digest возвращает тот же `OperationId`; другой digest возвращает `CONFLICT`. Перед выдачей существующей операции отдельно проверяются полномочия на неё. Вызовы authority и ожидание выполняются вне блокировки реестра.

Версионированный SHA-256 с разделителем области `biometric.verify.idempotency.v1\0` включает разрешённого субъекта, аутентификатор, цель из grant, исходные селектор и подсказку цели и `authority_request_ref`. Переменные поля кодируются с длиной u32 big-endian. Записи проверки ограничены 4096 ключами на соединение. Когда реестр активных операций заполнен, платформа может заменить безопасно завершённую операцию компактным снимком и сохранить её ключ. Такой снимок и ключ доступны до закрытия соединения. После пяти минут от исходного дедлайна платформа может освободить запись. Совпадающий ключ в этом окне не запускает работу повторно. После истечения окна новая авторизованная проверка может использовать ключ снова. История вытесненной операции восстанавливается через снимок и `Gap`. При исчерпании ёмкости новыми ключами платформа возвращает `LIMIT_EXCEEDED`. Идемпотентность мутаций остаётся устойчивой и не зависит от этой памяти.

#### Точный договор

1. Учёт принадлежит владельцу операций и действует только в текущем процессе;
   не сохранять записи проверки на диск и не восстанавливать после перезапуска.
   Запись состоит из доверенной привязки вызывающего, ключа, digest и номера
   существующей операции. Ключ в таблице — пара `(caller_binding, key)`;
   субъект и вид операции входят в digest. Это обеспечивает `CONFLICT` при
   повторном использовании того же ключа с иными параметрами, как требует 07.
2. `caller_binding` выдаёт доверенный адаптер полномочий для данной транзакции;
   UID, PID, имя клиента и данные тела запроса не являются его заменой. Пока
   интерфейс проверенного binding не установлен, повтор и создание отказывают
   `Unavailable`. Равенство одной лишь `CallerContext.opaque` не считать
   доказательством тождества транзакции. Перед возвратом существующей операции
   отдельно проверять полномочия вызывающего на её получение.
3. Предлагаемая версия канонического digest — SHA-256 от последовательности с
   разделением области `biometric.verify.idempotency.v1\0`, затем канонический
   `SubjectRef` (32 байта), `AuthenticatorId` (16 байт), цель из **grant**
   (явный стабильный код), вид селектора (явный код) и его исходное значение,
   исходная подсказка цели (явный код) и `authority_request_ref`. Переменные
   поля кодировать без нормализации: четырёхбайтовая беззнаковая длина в сетевом
   порядке, затем байты; у селектора текущего субъекта значение пусто. Это
   предложение определяет, какие отличия считаются конфликтом; оно не утверждает,
   что клиентские поля являются полномочием. Номер grant, срок, эпоха устройства
   и текущий состав регистраций в digest не включать: они не параметры запроса.
   Явно выделить версию кодирования; не полагаться на layout Rust enum или
   порядок полей protobuf. Не выводить исходные данные и digest в журналы.
4. Повтор сначала проходит проверку identity и policy, включая актуальность и
   отзыв. При том же binding и ключе совпадающий digest возвращает тот же
   `OperationId` без создания актора и без повторной аппаратной команды;
   несовпадение возвращает `CONFLICT`. При другом binding ключ независим, но
   не даёт права получить чужую операцию. Ссылка на запрос не является правом.
5. Проверка отсутствия ключа, выделение места, создание актора и публикация
   записи — одна неделимая операция владельца реестра. Никакие вызовы authority,
   ввод-вывод или `await` не выполняются под блокировкой. Отказ до публикации не
   оставляет ни записи, ни доступного актора; после публикации отмена вызывающего
   не удаляет запись и не разрешает повторный запуск. Следующий владелец продолжает
   уже зарегистрированную операцию. Конкурирующие повторы получают одну операцию.
6. Учёт ограничен числом записей, задаваемым административной конфигурацией;
   при заполнении новые ключи получают `LIMIT_EXCEEDED`, существующие по-прежнему
   сопоставляются. Записи не удаляются по сроку grant или завершению операции в
   пределах процесса: преждевременное удаление может запустить ту же просьбу
   повторно. При иной загрузке старые операции не действуют. Удержание записей
   по номеру операции требует сохранять доступный метаданный результат в течение
   того же срока; если это невозможно, повтор получает `Unavailable`, никогда
   не создаёт новую операцию автоматически.
7. Это решение не определяет лимит четырёх активных/ожидающих операций на
   соединение, управление очередью аутентификатора и освобождение мест; для них
   нужен отдельный договор. Публичный запуск остаётся закрыт без настоящих
   источников identity/policy, проверенного провайдера и остальных обязательных
   зависимостей.

#### Проверки после принятия

Одновременные одинаковые запросы создают один актор; другой digest конфликтует;
разные binding не раскрывают операции друг друга; различия каждого поля
селектор/цель/ref/субъект/аутентификатор проверяются отдельно. Проверить отказ
identity и policy, отзыв, истечение, переполнение, сбой создания и повтор после
сбоя, отмену вызывающего между стадиями, завершённую операцию и другой boot.
Проверить через настоящий `SOCK_SEQPACKET`, что при отказе провайдер не вызван.
Мутации используют свой durable договор и этими тестами не подменяются.

<a id="linux-generic-call-context"></a>

### Владелец транзакции PAM в Linux

В Linux демон выдаёт уникальную внутреннюю транзакцию после подтверждения соединения. PAM удерживает её и соединение в объекте одного вызова. Источник личности и политика независимо подтверждают вызывающего, субъекта, потребителя и цель. Транзакция связывает результат с исходным живым вызовом при проверенном отсутствии отзыва. Из этого ADR принято только изменение владельца транзакции B-04.

<a id="shared-client-transport"></a>

### Общий клиентский транспорт для PAM и CLI

CLI и PAM используют общий модуль Linux IPC для `carosign.client.v1`. Он проверяет защищённость подключения, размер и целостность пакета, отсутствие лишних FD, версию, корреляцию request ID, допустимость oneof и сроки. PAM отправляет запрос только при наличии доверенного контекста и выбранного аутентификатора. Иначе он возвращает `PAM_AUTHINFO_UNAVAIL`.

#### Точный договор

1. Вынести общий клиентский транспорт в отдельный модуль соответствующего
   Linux IPC crate. CLI и PAM используют один код; новый интерфейс описывает
   соединение и проверенные кадры существующего `carosign.client.v1` и не меняет
   wire. Источник случайных идентификаторов и ограничения протокола общие.
2. Проверять размер и целостность пакета, отсутствие лишних FD, согласованную
   версию, request ID, допустимость поля oneof и таймауты. Полученные данные
   остаются данными службы; связь сама по себе не предоставляет полномочий.
   Сокет и его окружение должны быть защищены установкой и проверены при
   подключении, а не считаться доверенными только по имени пути.
3. Отдельный слой PAM формирует запрос проверки только после определения
   доверенного контекста и выбора аутентификатора. При их отсутствии возвращать
   `PAM_AUTHINFO_UNAVAIL`, не отправлять вымышленный BeginVerification и не
   выдавать `PAM_SUCCESS`. Имя пользователя остаётся подсказкой.
4. PAM и CLI не получают аппаратные ресурсы; аппаратные провайдеры остаются
   отдельными проектами. Этот ADR не определяет конкретный способ биометрии.

#### Проверки после принятия

Оба клиента проходят Hello через настоящий `SOCK_SEQPACKET`; проверить неверный
request ID, неподдерживаемую версию, повторные oneof, усечение кадра и ancillary
data, закрытие, зависший сервер и отсутствие выдачи успеха PAM на любую ошибку.
Отдельно проверить жизненный цикл сокета и подмену сервера в системном окружении.

<a id="public-health-snapshot"></a>

### Публичный снимок состояния службы

`GetHealth` получает согласованный снимок существующего `Health` от владельца в корне сборки. Любое неизвестное обязательное поле даёт `UNAVAILABLE`. Нули основаны на подтверждённом учёте; число закрытых областей сообщает владелец восстановления. `service_ready` требует завершённого запуска, опубликованного слушателя и установленной безопасной политики. `ListAuthenticators` отдельно проверяет полномочия и инвентарь.

#### Точный договор

1. Добавить узкий источник снимка состояния, который принадлежит composition
   root службы. Обработчик не опрашивает провайдера и не получает аппаратных
   полномочий. Снимок содержит только поля существующего `Health`.
2. Источник возвращает `UNAVAILABLE`, если хотя бы одно обязательное поле
   нельзя достоверно получить. Нулевой счётчик разрешён лишь по подтверждённому
   пустому каталогу или результату актуального учёта, а не по отсутствию данных.
   Счётчик областей сверки поступает от владельца восстановления.
3. `service_ready` истинен только после выполнения шагов запуска документа 05,
   публикации слушателя и установления безопасной политики; области, закрытые
   для сверки, не представляются готовыми. Доступность устройства и проверка
   доказательств не выводятся из готовности службы. При изменении состояния
   поставщик предоставляет актуальный согласованный снимок либо отказывает;
   неподтверждённая устаревшая копия не выдаётся за текущую.
4. Тестовый composition вправе предоставлять заранее определённый снимок через
   явно подставной источник. Это не production bootstrap, не источник прав и не
   свидетельство готовности платформы. В production источника по умолчанию,
   возвращающего фиктивные нули или готовность, нет.
5. `ListAuthenticators` остаётся отдельной операцией: список требует проверки
   полномочий наблюдателя и подтверждённого инвентаря. Данный ADR не разрешает
   выдавать список только по доступности публичного health.

#### Проверки после принятия

Настоящий `carosignctl` получает снимок от отдельного тестового процесса по
`SOCK_SEQPACKET`; проверить все поля и идентификатор запроса. При недоступном
источнике — `UNAVAILABLE`, без фальшивого health. Проверить изменение снимка,
невозможность вывести ready до установления политики, закрытые области,
ошибку источника и отсутствие обхода policy при запросе списка.

### Состояние провайдеров

`Authenticators.providers` присутствует в режиме обнаружения публикаций, в том числе при пустом списке. Каждый элемент соответствует имени записи в разрешённом каталоге провайдеров. `reachable` означает, что проверенный сокет провайдера вернул корректный ответ инвентаризации. Ответ может быть пустым. Готовность устройства определяет `AuthenticatorDescriptor.available`.

`authenticator_ids` ссылается на элементы `entries` из того же ответа. Известное платформе устройство остаётся в группе своего провайдера со статусом недоступности после отключения. Сведения о провайдерах защищены той же политикой инвентаризации, что и устройства. В режиме прямого запуска воркера поле `providers` отсутствует.

### Выбор регистрации при проверке

`HelloAck.verification_enrollment_selector` сообщает поддержку полей
`BeginVerification.enrollment` и `BeginVerification.report_matched_enrollment`.
Клиент проверяет этот флаг перед отправкой запроса с любым из этих полей.

При выборе регистрации платформа сверяет ID аутентификатора, ID записи
и поколение с авторизованным набором записей субъекта. Провайдер получает
только выбранную запись. Если подходящей записи нет, операция не начинается.
Ссылка и запрос раскрытия входят в идентичность запроса проверки для
идемпотентности.

Подтверждённая ссылка `VerificationTerminal.matched_enrollment` передаётся
по запросу в положительном итоге исходному клиенту с полномочием
`SessionLogin`, которое также даёт доступ к списку регистраций субъекта.
Снимки, повторное наблюдение и итоги иных целей её не раскрывают.
