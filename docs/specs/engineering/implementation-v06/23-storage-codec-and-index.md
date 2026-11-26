# 23. Exact stable encoding for redb metadata keys and values

This document supplements [10](10-persistence-and-recovery.md) and the authoritative `carosign/storage/v1/storage.proto`. Rust object serialization (`bincode`, `serde_json`) is **not** the persistent schema. Core IDs are exactly 16 nonzero bytes.

## Key bytes (no length ambiguity)

| redb table | Key encoding | Length |
|---|---|---:|
| `authenticators` | `AuthenticatorId` | 16 |
| `enrollments` | `EnrollmentId` | 16 |
| `enrollment_index` | `AuthorityId  +  SubjectId  +  AuthenticatorId  +  EnrollmentId` | 64 |
| `mutations` | `MutationId` | 16 |
| `mutation_scope_lock` | `AuthorityId  +  SubjectId  +  AuthenticatorId` | 48 |
| `effect_outbox` | `OperationId  +  transition_index(u64 BE)  +  effect_position(u32 BE)` | 28 |
| `idempotency` | `caller_binding_digest(32)  +  idempotency_key(16)` | 48 |
| `provider_state` | `PackageId  +  AuthenticatorId` | 32 |
| `packages` | normalized reverse-DNS package name in UTF-8/ASCII | 1..128 |
| `template_blobs` | `EnrollmentId  +  generation(u64 BE)` (canonical; mirrored in value and validated) | 24 |

` + ` in the table means raw byte concatenation, **not** a stored delimiter or the 0x7f byte. The ASCII-looking notation here is only for readability. Prefix scans on first 48 bytes of `enrollment_index` provide subject+authenticator list; never scan another subject without authority check.

## Durable value envelope (binary)

```
byte 0..3    ASCII "BIO1" (4 bytes)
byte 4..7    protobuf payload length, u32 little endian
byte 8..N    encoded carosign.storage.v1.StoredValue
next 32      SHA-256 digest of the protobuf payload only
```

Total value length must equal `8 + length + 32`. For metadata `length` is 1..65536 bytes; for `template_blobs` `length` is 1..8388608 bytes, enforced only for authenticated encrypted blob values. The public IPC frame limit remains 65536 bytes. Parser verifies magic, exact length, SHA-256, protobuf parsing, `schema_version==1`, required fields and semantic validation. SHA-256 here detects accidental corruption but **does not** authenticate against an attacker who can rewrite the database. Cryptographic anti-rollback and protected metadata integrity require separate trusted anchor policy.

`meta` contains a versioned schema marker and migration state using a separate restricted well-known key, not mistaken for `StoredValue` of enrollment. If schema unknown, refuse mutation and retain bytes.

## Indexed record invariants

- On insert/remove/replace, update `enrollments` and its index in **same redb transaction**.
- Under mutation scope lock, at most one non-terminal MutationId for `(AuthorityId, SubjectId, AuthenticatorId)`. If stale lock exists, enter reconciliation and refuse new mutation.
- `expected_generation` is CAS against exact current EnrollmentRef before provider dispatch and again before Registry commit; conflict after provider changed hardware becomes reconciliation, never silent overwrite.
- Outbox entries for mutation exist in same transaction as `DISPATCH_ARMED` record. Exactly one command ID per mutation. Never duplicate send because status remains ARMED after restart.
- `mutation_scope_lock` is not removed until VerifiedCompleted or IndependentlyConfirmedNoChange and durable terminal phase.
- `provider_state` can be unavailable when package removed; keep metadata and template refs.

## Record validation

For `MutationRecord`, `worker_instance_id`/`command_id` may be empty in `Prepared`, but must be 16B at `DispatchArmed` or later. `provider_observation_digest` 32 bytes only when observation present. `EnrollmentRecord` requires `SubjectRef`, `EnrollmentRef`, supported status and bounded format/opaque ref. `IdempotencyRecord` has fixed 32-byte hashes and 16-byte IDs. Unexpected/incomplete fields → corrupted record. Do not patch/reinterpret silently.

## Durability

No I/O within a redb write transaction except DB operations; obtain `DurableReceipt` only once committed with the backend's strongest documented durability mode. System metadata DB file and filesystem must support required durability; no claim that userspace `fsync` can make noncompliant storage hardware safe. Crash/power-loss tests needed on target filesystem before production qualification.

### Canonical key vs mirrored value identity (v0.6.1 clarification)

For `template_blobs`, the **only redb primary key** is `EnrollmentId[16] || generation.to_be_bytes()[8]` (24 bytes). `StoredValue.template_blob.enrollment_id` and `.generation` are deliberately redundant **identity mirrors**, not a secondary key and not an alternative location of truth. On write, derive both mirrors from the single validated key. On read, reject `CorruptRecord` if `record.enrollment_id != key[0..16]` or `record.generation != u64::from_be_bytes(key[16..24])`. Validate nonzero 16B EnrollmentId and generation >= 1. There must never be a best-effort rewrite, silent fallback, or implicit rekey. The template is decrypted only after validating key, record mirrors and Registry ownership/format binding in the same consistent database snapshot; AAD is reconstructed from that validated binding. A mismatch is storage corruption and prevents matching. Test D-11.

A new generation creates a different 24-byte key. Overwriting the value of an existing key with a different record ID/generation is prohibited. The same invariant applies to every read path including recovery and migration.

### v0.6.2 terminal outcome cross-field constraint

`MutationRecord.terminal_outcome` is `MUTATION_TERMINAL_OUTCOME_UNSPECIFIED` unless `phase == MUTATION_COMPLETED`. Exactly when phase is completed it MUST be `MUTATION_TERMINAL_COMMITTED` or `MUTATION_TERMINAL_NO_CHANGE`. `MUTATION_NO_CHANGE` phase is a pre-cleanup stage, never a safe terminal result. Durable finalization and scope-lock release must be one metadata transaction. Read, replay, migration and public `GetOperation` paths validate both fields; incomplete/contradictory records fail closed. Tests B-01f/B-02f. See [27](27-v062-engine-conflict-resolutions.md).

### v0.6.4 optional-template interpretation

`template_blobs` has the stated canonical schema **if an approved provider uses the optional platform vault**. It is neither the universal enrollment store nor a requirement on hardware-owned templates. Provider-owned TA/chip records may have only an `EnrollmentRecord` with an opaque binding; absence of `template_blobs` is normal. Core may not deserialize carosign-template content. See [29](29-v064-provider-owned-matching.md).

### v0.6.5: canonical plan storage v2

Use `MutationRecord.plan` from `storage.proto` and stable canonical plan digest in [33](33-v065-durable-mutation-plan.md). Its identity/snapshot bytes are stored as versioned protobuf in the `mutations` record, with canonical digest byte construction independent of Protobuf field ordering. `schema_version=2` for new writes. The mutation scope lock is released only after verified durable terminal outcome plus safe cleanup, never by a no-change inference from a missing provider ref. Malformed v1 armed records remain quarantined; no fabricated upgrade.

### v0.6.6 storage schema v3

New mutation records and Registry enrollment records require the custody discriminator, and the selected old target participates in the mutation scope digest; `MutationRecord.target_enrollment` is a checked mirror for REPLACE/REMOVE, not a second source. v2 schema, if encountered, follows the explicit quarantine/migration rules in [34](34-v066-enrollment-command-projection.md).

## Маркер схемы и метаданные установки

<a id="storage-initialization"></a>

### Создание хранилища v3 и маркер состояния схемы

`meta/biometric.schema` хранит 12 байтов: ASCII `BSC1`, затем версию схемы и состояние миграции как два u32 big-endian. Договор v3 использует пару `(3, 0)`; для новой базы первого выпуска действует v4 по документу 35. Явное создание нового файла фиксирует таблицы и маркер долговечно. Обычный запуск проверяет маркер и снимок и возвращает `NotReady` при неизвестном, неполном или повреждённом состоянии, сохраняя данные.

#### Точный договор

* В таблице `meta` зарезервировать ключ ASCII `biometric.schema`.
* Значение — ровно 12 байтов: ASCII `BSC1`, затем версия схемы как `u32` в
  big-endian, затем состояние миграции как `u32` в big-endian. Значение
  `3, 0` означает схему v3 без незавершённой миграции. Все прочие значения,
  длины, магии и версии отвергаются как неизвестные. SHA-256 для записей
  `StoredValue` остаётся неизменным; маркер не является `StoredValue` и не
  утверждает защищённость от подмены или отката.
* Первичная инициализация исторической схемы v3 — только явной
  административной командой для нового файла: `create_new`, создание нормативных таблиц и запись маркера одной
  долговечной транзакцией. Существующий путь не открывается на запись и не
  перезаписывается. При отказе и прерывании возникший файл не считается
  готовым до повторного независимого открытия и проверки.
* Обычный запуск не создаёт базу v3. При открытии существующей базы без
  ключа, с неизвестным ключом, версией или состоянием результат — `NotReady`,
  байты сохраняются. Временный формат и любые старые v1/v2 не мигрируются
  автоматически. Завершение миграции потребует отдельного договора и плана
  отката; неподтверждённые мутации сохраняются в карантине.
* После подтверждения маркера запуск выполняет `StartupScan::scan_v3_with_scopes`
  и дополнительные обязательные проверки до публикации. Сам маркер и успешный
  scan не дают полномочия провайдеру, не подтверждают аппаратное состояние и
  не заменяют проверки остальных портов. Заблокированные области остаются
  недоступными; критическая ошибка восстановления означает `NotReady`.

#### Проверка реализации после принятия

Первое создание и повторное открытие, отказ при существующем пути, отсутствие
маркера, неизвестная версия и состояние, повреждённая длина или магия,
прерывание до фиксации, конфликт индекса/журнала, незавершённая мутация,
сохранение старой базы без записи, повторный запуск после потери подтверждения
фиксации. Проверки отказа питания на целевой файловой системе остаются
отдельным условием эксплуатационной квалификации.

#### Границы решения

Решение определяет внутренний маркер и первичное создание, а не формат
биометрических шаблонов, миграцию существующих баз, аппаратного провайдера,
источник полномочий, проверку доказательств или политику выдачи ресурсов.

<a id="installation-record-storage"></a>

### Подтверждённая установка в профиле с отдельной записью

Профиль с подтверждённой записью установки хранит `provider_installations` в той же redb-базе. Ключ — `PackageId[16] || ProfileId[16]`; значение использует оболочку `BIO1` и `StoredValue.provider_installation` (тег 18, схема v4). Идентичность, версия и профиль согласуются с Registry в одном снимке; публикация и отзыв используют проверяемые монотонные ревизии. Документ 35 разрешает первый Linux-выпуск без обязательной записи подтверждённой установки.

#### Точный договор

1. Добавить таблицу `provider_installations` **в тот же redb-файл**, что
   Registry, MutationJournal и Outbox. Ключ — `PackageId[16] || ProfileId[16]`
   (32 байта, без разделителей). `ProviderId` и оба идентификатора ключа
   зеркалируются в значении и проверяются при каждом чтении. Нулевые ID,
   несовпадающие зеркала и дубли — повреждение, не новая установка.
2. Добавить в `carosign.storage.v1.StoredValue` новый, ранее не использованный тег
   `provider_installation = 18`; существующие теги и поля `PackageRecord` не
   переопределять. Сообщение `ProviderInstallationRecord` хранит:
   `package_id`, `provider_id`, `profile_id`; каноническое имя пакета,
   версию и имя профиля; относительное имя каталога версии внутри защищённого
   системного корня; идентичность открытого каталога `(device, inode)`;
   утверждённый основной fingerprint подписанта и идентификатор политики
   ключей; положительную монотонную ревизию записи и состояние `ACTIVE` или
   `REVOKED`; пути policy, манифеста, профиля и executable; список файлов с
   относительным путём, ролью и 32-байтным измерением SHA-256 **fs-verity**.
   Имена и версия сверяются с каноническим `PackageRecord`, а профиль — с его
   `approved_profiles`. Пути проходят тот же строгий проверяющий разбор, что
   при открытии файла; абсолютные, пустые, `.`/`..`, дубли путей и ролей
   обязательных четырёх файлов отвергаются. Runtime-роль допускает несколько
   файлов с различными путями. Максимум 128 файлов всего; каждый путь — не
   более 1024 байт; полный сериализованный `StoredValue` — не более 65536
   байт. Fingerprint задаётся как проверенный основной OpenPGP fingerprint:
   20 или 32 бинарных байта с явно указанной версией ключа; произвольная
   строка и fingerprint подключа не считаются основным ключом. Идентификатор
   политики ключей — непрозрачный системный ID в 16 ненулевых байт, не секрет.
   PID, полученные от работника данные, биометрический материал и ссылки на
   открытые FD в записи не хранятся.
3. Все значения, включая новую запись, используют существующий `BIO1` envelope
   с длиной и SHA-256 согласно документу 23; payload — protobuf `StoredValue`.
   Для новой установки маркер базы и новые `StoredValue` создаются сразу с
   **schema version 4**. При переводе существующей поддерживаемой базы на v4
   маркер и новые `StoredValue` меняются только вместе с одобренной миграцией. Временное
   сосуществование v3/v4 разрешено только во время журналируемой миграции;
   обычный режим v4 не выдаёт частично обработанные значения. Существующие
   теги, семантика и содержимое `PackageRecord`, enrollment, mutation, outbox и
   другие записи v3 сохраняются при переводе; для записей, не требующих
   изменения полей, меняется только версия protobuf-оболочки с пересчётом
   внешнего envelope. Не применять незнакомые версии или поля, не превращать
   ошибку разбора в пустую базу. Контрольная сумма не даёт защиты от злонамеренной
   подмены или отката: системная защита целостности и антиоткат — отдельная
   зависимость, их отсутствие не заявлять как защищённое состояние.
   Таблица `provider_installations` никогда не создаётся и не используется под
   действующим маркером v3: иначе старый бинарник мог бы игнорировать критичные
   сведения. До миграции проверить, что применяемый v3-бинарник действительно
   отвергает маркер v4 и незавершённую миграцию до публикации слушателя; без
   этой проверки обновление запрещено. После смены маркера запуск любого старого
   бинарника и откат его пакета должны отказать без изменения базы. Восстановление
   v3 из резервной копии не является обычным downgrade: оно требует отдельной
   остановки всех служб, оценки потери новых данных и явного административного
   восстановления; оно не доказывает актуальность ключей или полномочий.
4. Источник записи — только системный установщик после проверки подписи и
   фиксации `fs-verity` по удержанным FD. При подготовке операции он собирает
   полную запись вне транзакции; worker, текст манифеста, UID клиента и
   содержимое redb не создают подтверждения происхождения. В одной write-транзакции
   он перечитывает `packages` и текущую установку, проверяет полномочие системного
   установщика, допустимость имени/версии/профиля, стабильность ID, ожидаемую
   ревизию и запрет конфликтующего владельца или подписанта; затем атомарно
   обновляет `PackageRecord` и запись установки. Повышение ревизии проверяется на
   переполнение. Публикация только после `commit_durable`; ошибка фиксации
   означает неопределённый исход и требует чтения, не автоматической публикации
   заново. Смену подписанта или несовместимую переустановку допускает только
   отдельное решение администратора по документу 24. Допуск к этому API не
   выводится из одного UID или root-owned файла.
5. Отзыв атомарно меняет `ACTIVE → REVOKED` и ревизию в той же базе; не удалять
   регистрацию, шаблоны, пакетную идентичность и историю принадлежности. При
   удалении пакета сохранить `PackageRecord` с пустой установленной версией и
   неизменными ID/профилями, а запись установки перевести в `REVOKED` в той же
   транзакции. После отзыва новые запуски запрещены; уже выданные ресурсы
   освобождаются по действующему договору отзыва/уборки. Любой повтор публикации
   с устаревшей ревизией получает конфликт, а не перезаписывает отзыв.
6. При запуске supervisor читает `PackageRecord` и установку из **одного**
   read-снимка, проверяет их тождество, активность и ревизию, открывает каталог
   от системного корня и сверяет удержанный root с записью. Для каждого файла
   открывает и удерживает FD без ссылок и сверяет fs-verity с записью. До допуска
   перечитывает актуальную запись и политику на отзыв; отсутствие источника,
   неопределённость, ошибка или несовпадение дают отказ. Открытие каталога по
   имени и совпадение `(device,inode)` не доказывают подпись и не разрешают
   обходить системный источник. Запись сама по себе не выдаёт ресурсы, не
   подтверждает успешное завершение оборудования и не заменяет policy.

#### Миграция и восстановление

* Служба Linux инициализирует отсутствующую базу v4 при запуске в доверенном
  каталоге. Ручное создание также допускается только для отсутствующего пути.
  Открытие существующей базы сохраняет файл при повреждении или неизвестной
  схеме. Перед миграцией v3 → v4 получить проверяемую резервную копию и
  остановить новые операции; миграцию запрещать при активной или неразрешённой
  аппаратной мутации. Существующую v3 не перезаписывать без журнала.
* В **той же** базе зафиксировать `MigrationPending` с исходной и целевой
  версиями и идентификатором, затем `Staged`, затем одной durable транзакцией
  опубликовать все обработанные значения, новую таблицу и маркер v4 и отметить
  `Committed`. Реализация может использовать ограниченную staging-таблицу для
  больших баз; partial v4 не разрешает запуск или мутацию. Если отрыв записи от
  маркера допускает неоднозначность — восстановление отказывает и сохраняет
  все байты. При `Pending`/`Staged` автоматически не продолжать выдачу
  полномочий; запустить явное восстановление или откат по проверенному журналу.
* Предыдущие `PackageRecord` с непустой версией не доказывают источник:
  миграция не создаёт для них `ACTIVE` installation record. Сохранить прежние
  ID и регистрации; провайдер недоступен, пока системный установщик не
  переутвердит конкретный подписанный артефакт и не подтвердит совместимость.
  Шаблоны, provider state, блокировки и неразрешённые мутации не удаляются.
* До каждой необратимой операции с внешней ФС использовать отдельные стадии
  установщика: `VerifiedArchive → StagedFiles → VerityConfirmed → Published`.
  Действия ФС происходят вне redb write-транзакции: ACID не охватывает ФС и БД.
  После сбоя незарегистрированный staging-каталог не становится активным;
  запись без всех открываемых и совпадающих файлов не даёт допуск. Очистку
  staging выполнять только после проверки принадлежности и отсутствия активных
  пользователей. Изменение или удаление каталога работающей версии до
  подтверждённого покоя запрещено принятым ADR supervisor.

#### Проверки до подключения

Проверить строгий codec: неизвестная версия/роль, повреждение и усечение,
дубликаты, неверный ключ и зеркала, превышения размеров, неподтверждённый
подписант, несовпадение имени/версии/профиля/ID и переполнение ревизии.
Транзакционные испытания: конфликт конкурирующих установок, отзыв во время
проверки, crash до/после публикации, неоднозначный commit, переоткрытие,
отсутствующий или заменённый runtime, несовпадение fs-verity каждого файла,
удаление пакета с сохранением регистраций и устаревшая ревизия после отзыва.
Миграционные испытания: v3 с активными и повреждёнными областями, каждый
промежуточный маркер, отсутствие автоматического доверия старой установке,
проверка резервной копии, отказ при неизвестной схеме и запуск настоящего
прежнего бинарника на v4/незавершённой миграции без открытия публичного сокета
и без записи. Проверить отсутствие неразрешённого создания v4-таблицы под
маркером v3 и одновременный старт старой и новой версий с общим lock. Отдельно испытать
реальный сбой питания на целевой ФС; обычные unit-тесты его не подтверждают.

До реализации и проверок это предложение не разрешает менять схему v3,
подключать отдельную экспериментальную базу или выдавать происхождение пакета
по совпадению хеша.
