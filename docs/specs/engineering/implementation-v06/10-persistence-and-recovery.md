# 10. Durable metadata, outbox, reconciliation and transactional boundaries

**Уточнение первого выпуска:** [35](35-first-release-scope.md) фиксирует
отсутствие выпущенных v3-установок и новую базу v4. Требования миграции ниже
применяются к будущим обновлениям поддерживаемых данных, а не служат условием
первой установки. Защита существующих или неизвестных данных сохраняется.

**Уточнение совместимости хранилища:** текущий формат, включая маркер схемы,
ключи, таблицы и кодирование записей, сохраняется при переименовании модулей.
Названия рабочих модулей и публичных операций описывают назначение, а не номер
внутренней схемы. Новые версии платформы обязаны открывать все поддерживаемые
форматы предыдущих выпусков без потери регистраций и незавершённых операций.
Обратно совместимые изменения предпочитаются преобразованиям данных; если
преобразование необходимо, оно оформляется отдельным явным шагом миграции с
проверкой исходной схемы, восстановлением после прерывания и испытанием на
сохранённой базе предыдущего выпуска. Неизвестная будущая схема не меняется и
отвергается; возможность запуска старого бинарника после обновления формата
обещается только при отдельно подтверждённой совместимости записи.

**Baseline:** single redb metadata database (Registry + MutationJournal + Command Outbox), managed by `carosign-persistence`; `template store` remains separate and may live in TA/RPMB or encrypted provider storage. No ACID transaction spans a TA, another process, or external filesystem. Do not implement a cross-hardware rollback fantasy.

## Tables, keys and values

| Table (schema v1) | Key | Value | Owner |
|---|---|---|---|
| `meta` | static enum key | schema revision, migration status, installation identity | persistence |
| `authenticators` | AuthenticatorId(16) | provider binding, device identity fingerprint, epoch, availability | registry |
| `enrollments` | EnrollmentId(16) | SubjectRef, AuthenticatorId, generation, label, opaque provider ref, state | registry |
| `enrollment_index` | `(AuthorityId,SubjectId,AuthenticatorId,EnrollmentId)` | EnrollmentId | registry |
| `mutations` | MutationId(16) | target, expected generation, phase, recovery reference, checksum | journal |
| `mutation_scope_lock` | `(AuthorityId,SubjectId,AuthenticatorId)` | MutationId | journal |
| `effect_outbox` | `(OperationId,TransitionIndex,EffectPosition)` | effect kind, armed/observed phase, command id | effect runner |
| `idempotency` | `(CallerBindingHash,IdempotencyKey)` | request digest, op/mutation ID, retention | runtime |
| `provider_state` | `(PackageId,AuthenticatorId)` | state format, opaque references, migration status | registry |
| `packages` | package name (reverse-DNS, canonical ASCII) | assigned PackageId/ProviderId + installed state | provider manager |
| `template_blobs` | `EnrollmentId[16] || generation(u64 BE)[8]` | `StoredValue.template_blob`; encrypted blob plus **mirrored** EnrollmentId/generation, both REQUIRED to equal the key | template repository |

Keys are fixed-endian canonical bytes. Value encoding uses a **separate versioned storage schema**; do not serialize raw Rust struct layout or use unversioned bincode as the durable contract. Baseline durable value representation: protobuf messages in internal `schema_version=1` envelope with checksums and **bounded** lengths; metadata protection/rollback assurance comes separately from trust anchor, not CRC. Changes in storage schemas require explicit migration and journal recovery plan.

## Write transaction API

```rust
pub trait MetadataStore: Send + Sync {
    fn begin_read(&self) -> Result<ReadTx<'_>, StorageError>;
    fn begin_write(&self) -> Result<WriteTx<'_>, StorageError>;
}
```

`ReadTx`/`WriteTx` are **internal types** with no `async fn` and may not cross `.await`. At most one write transaction per mutation step. A successful returned `DurableReceipt` means the committed data met configured durability guarantees of the backend. Do not dispatch mutating hardware commands on `Ok` from a merely buffered/non-durable write. If backend cannot establish durable commit under requested policy → refuse mutation; do not silently downgrade.

## Mutation boundaries and exact protocol

1. `prepare`: transaction inserts MutationRecord(`Prepared`), scope lock, idempotency record; commit durably; no side effect on hardware before success.
2. Acquire lease and revalidate management grant. If denied, record `NoChange` and release scope lock (after proving no dispatch).
3. `arm`: transaction switches mutation to `DispatchArmed`, creates outbox record `(MutationId,CommandId,OperationId,scope,epoch,worker_instance)`, commits **durably**. Only then side-effect `SendMutatingCommand` is permitted.
4. Send exactly once from that in-memory active operation. Do **not** automatically replay `DispatchArmed` record on startup; it may have already executed hardware mutation.
5. Observe outcome. `ProviderCompleted` records observation; if provider or protocol requires separate state verification, execute safe `QueryMutationState` and validate independent result.
6. `commit`: after observed or independently verified outcome, execute **one of two fixed atomic transactions**. `CommitDesiredRegistryAndJournal`: apply desired enrollment/index change with CAS, record `MUTATION_REGISTRY_COMMITTED` and outbox status. `CommitNoChangeAndJournal`: make **no enrollment/index changes**, but atomically record verified pre-state, mutation `MUTATION_NO_CHANGE` and outbox outcome. Both retain the scope lock, recheck expected enrollment generation under the scope lock, and persist result evidence/provenance. A mismatched CAS after device-side change returns to reconciliation, never overwrites a newer registration. This applies equally after `Reconciling`.
7. `finish`: after confirmed resource quiescence, durably mark `MUTATION_COMPLETED` with `terminal_outcome=COMMITTED|NO_CHANGE` and release scope lock **in one metadata transaction**. Notify only after confirmed durability. Failure to persist finalization leaves scope blocked for recovery, never a fabricated terminal result.

**Crash windows:**

| Window | Recovery |
|---|---|
| Before step 1 durable | no mutation started; safe to refuse |
| `Prepared`, no `DispatchArmed` | confirmed no dispatch; safe no-change cleanup |
| After `DispatchArmed`, before/after IPC send | treat as `NeedsReconciliation` regardless of send ACK |
| After provider success, before metadata CAS | query actual state, then reconcile registry |
| After Registry CAS, before journal Completed | verify current binding and completion, close pending journal |
| During removal | same as enrollment; success of delete != proof of physical secure erase |

No re-execution of `Enroll/Remove` when operation already armed. Even read-only query must be confirmed side-effect free for the specific backend. If independent check unavailable, affected scope stays blocked and admin recovery is required.

## Reconciliation authority

`ReconciliationVerifier` must verify **actual** state using source independent enough from the lost worker event to satisfy provider-specific risk policy. For software-owned storage, atomic blob state+version may be sufficient; for TA-owned storage, query semantics must be established empirically. `Unknown`, malformed reply, changed device identity or epoch → never infer success. If actor/grant died, recovery continues as *maintenance*, not new user-authorized mutation, and must not reveal a result to unauthorized observer.

## Template custody

- Optional platform-managed vault **when explicitly requested by a provider**: encrypted `ProtectedTemplateRecord` in `template_blobs` **inside the same redb database**, supporting atomic ciphertext+Registry CAS+journal metadata commit when the provider-specific write/staging contract is established. `format_id`,`format_version`,`authenticator_id`,`subject`,`enrollment_id`,`generation` are authenticated as AAD; key through `KeyProvider`; plaintext exists only in restricted provider storage adapter, not in Registry or IPC. Template-record maximum 8 MiB, metadata-record maximum 64 KiB. An alternative external blob store requires separate accepted ADR with crash-safe staging protocol; **implementation agent must not invent it**.
- Provider-managed: only bounded `OpaqueEnrollmentRef` in Registry; TA owns templates. No attempt to open/export absent capabilities.
- Changing/removing provider packages preserves registry and blobs; uninstall never deletes them by default.
- Encryption at rest protects confidentiality/integrity under key custody assumptions; does **not** imply rollback prevention or resistance to local root on unverified boot.

## Recovery boot gate

Before public listener readiness: validate schema and DB integrity, scan `mutations`/`outbox`, lock uncertain scopes, then start workers for safe-query reconciliation only. Corrupt existing store is **not** reset as empty. Unknown state exposed as degraded/unavailable. No new mutation on blocked scope until trusted verifier clears it; normal operations on independent unaffected scopes may proceed.

## Migration

Storage schema upgrade is itself journaled (`MigrationPending → Staged → Committed`), with backups/rollback plan and crash recovery; not executed while provider mutation active. Provider package format migration independent of central schema; incompatible template format must fail activation or require explicit migration, never reinterpret bytes.

### Recovery without independently verifiable TA query (v0.6.1)

For Goodix/TB371FC no safe independent TA-state query is established in the supplied research. Missing capability => `QUERY_UNSUPPORTED`; an attempted non-conclusive safe query => `QUERY_UNKNOWN`. Both persist `NeedsReconciliation`, retain scope lock and **prevent using affected registration for verification**. Manager can keep unrelated scopes alive, but not infer hardware state from worker health. Unblock only after verified hardware-state evidence, verified destructive reprovision/clear supported by device and independent verification, or permanently disable/decommission the authenticator without falsely reporting restored state. Admin consent permits destructive remediation but **is not** state evidence. No known safe remediation => keep blocked; execution agent stops at `ARCHITECTURE_BLOCKER` for Goodix recovery. See [14](14-provider-integration-contracts.md) and [25](25-v061-conflict-resolutions.md).

### Reconciliation/metadata failures v0.6.2 — no implicit alternatives

A trusted `ReconciliationVerdict` has three cases: verified post-state → `Committing` with desired Registry CAS; verified *complete* original pre-state → `Committing` with metadata-only NoChange commit; indeterminate → remain `Reconciling` with scope lock. No `Reconciling → Terminal`. For `Remove`, verified **absence** of target is post-state (Committed), while verified preservation of target is pre-state (NoChange). For `Enroll`, verified desired registration is post-state (Committed), while verified untouched original state is pre-state (NoChange). Mismatched identity, generation, incomplete replacement or untrusted query result is neither.

After durable `DispatchArmed`, `MetadataFailed` is an uncertain metadata commit: engine enters `Reconciling`, blocks affected scope and may not resend device mutation. If DB is unavailable and `mark_uncertain` fails, report local `StorageUnavailable` without claiming durable repair, block any authorization on affected scope and inhibit service readiness as required. Once storage is accessible, read **actual durable state** (possibly `REGISTRY_COMMITTED` or `MUTATION_NO_CHANGE` from a commit whose ACK was lost), validate CAS/receipt against independently checked hardware as needed, and finalize without repeating device command. If underlying record corrupted, fail closed.

Stored `MUTATION_COMPLETED` must carry an explicit `terminal_outcome` distinguishing `COMMITTED` and `NO_CHANGE`; `MUTATION_NO_CHANGE` is an **intermediate persisted outcome awaiting cleanup**, not sufficient by itself for unlocked terminal state. Existing v0.6.1 durable schema was a draft, not a shipped frozen format; the new field is reflected in `storage.proto`. Existing deployed data, if any, requires an explicit migration rather than silently defaulting to success. See [27](27-v062-engine-conflict-resolutions.md).

### v0.6.3: immutable reconciliation context after process recovery

Engine `OperationBody::{Enrollment,Removal}` requires validated mutation plan state with the full original/precondition and target/desired postcondition as defined in [28](28-v063-operation-discriminant-and-send.md). `MutationRecord.kind` is a storage consistency check, not a runtime machine selector. Existing `MutationRecord.expected_generation`/`target_enrollment` **alone do not establish complete hardware pre-/post-state**, notably for `Enroll(replace)`. If a trusted comparison context is unavailable after restart, only `Indeterminate` is permitted and the affected scope stays blocked. The exact normative on-disk encoding for a full portable plan is **not established in v0.6.3**; an implementation that must persist/recover such a plan raises `ARCHITECTURE_BLOCKER` instead of inventing a schema or using a partial proxy.

### v0.6.4 — platform vault is optional; not a matching backend

The `template_blobs` redb table and `ProtectedTemplateRecord` define an **optional encrypted template vault for providers requesting external storage**. Although detailed here, this is not a mandatory baseline for every authenticator: TA/chip/provider-managed enrollment requires **no** `template_blobs` value. Runtime metadata Registry and mutation journal are mandatory; software templates, their serialization and matching are never interpreted by Core/Engine. Any service giving a worker access to vault blobs must be explicitly scoped per enrollment/subject/authenticator/generation and needs a separately approved vault protocol + durable staging contract before implementation. `CUSTODY_PLATFORM_MANAGED` identifies use of that service, **not host matching**; `CUSTODY_PROVIDER_MANAGED` means no platform vault required. See [29](29-v064-provider-owned-matching.md).

### v0.6.5: durable plan v2 is mandatory for new mutations

`MutationRecord.plan` is now a required immutable `MutationPlanRecord` persisted with `MUTATION_PREPARED`, with canonical logical binding snapshots, prospective enrollment ID/generation and desired transition predicate. `StoredValue.schema_version=2`; migration/nonterminal v1 rules and the exact digest are in [33](33-v065-durable-mutation-plan.md). Previous text identifying this as an *unspecified* on-disk format describes v0.6.3 history only. Plan recovery cannot replace an independent physical hardware witness; Goodix without safe query remains blocked. Do not serialize HAT/token, mutable DMA buffers or provider raw template into the plan.

### v0.6.6 durable enrollment projection amendment

Earlier plan-v2 wording is historic. All newly prepared mutation records are `StoredValue.schema_version=3`, with typed custody kind and `old_target` included in the canonical digest. Runtime's checked `CommandPlan` is a **volatile derived projection**, not stored as second source of truth. It must be constructed only under the matching scope lock from the immutable plan and verified full Registry snapshot. After durable arm, no automatic rebuild/resend is allowed. See [34](34-v066-enrollment-command-projection.md).

## Подтверждённый откат регистрации

При явной отмене источником подтверждения полного исходного состояния служит
доверенный результат транзакционного отката по
[контракту провайдера](06-provider-host-and-wire.md#транзакция-регистрации-и-явный-откат).
Правила атомарной записи `NoChange`, удержания блокировки до уборки и сверки
при неопределённости применяются в полном объёме.

## Условный договор миграции v3 → v4

<a id="v4-migration-journal"></a>

### Журнал и восстановление миграции метаданных v3 → v4

Для отдельно разрешённого перехода журнал `meta/biometric.migration.v3-v4` проходит `Pending → Staged → Committed`. Он связывает резервную копию и снимок staging с одним ID миграции; `BIO1` оборачивает 128-байтовый payload `BMIG`. После проверки резервной копии, системного ограждения и отказа старого бинарника перенос, маркер v4 и стадия `Committed` публикуются атомарно. При неопределённом commit читается устойчивая стадия; повтор по потере ответа запрещён. Документ 35 исключает эту миграцию из условий первого выпуска Linux.

#### Точный договор

1. Единственная запись журнала находится в `meta` по ключу
   `biometric.migration.v3-v4`. Её значение — обычный `BIO1` envelope с
   SHA-256; полезная нагрузка имеет отдельную фиксированную сигнатуру `BMIG`,
   версию кодека 1 и строгое перечисление стадий `Pending`, `Staged`,
   `Committed`. Хранятся ненулевой 16-байтовый ID миграции, точные номера
   исходной и целевой схемы, утверждённая длина и SHA-256 резервной копии,
   идентичность открытого файла копии `(device, inode)` и итоговое число и
   SHA-256 канонического снимка подготовленных записей. Числа кодируются
   big-endian; неизвестная версия, неверная длина, ненулевой зарезервированный
   байт, лишние байты, нулевой ID или неправильное сочетание полей — отказ.
   Точные допустимые сочетания задаются кодеком по стадии: у `Pending` нет
   утверждённого снимка staging, у `Staged` и `Committed` он обязателен.
   Контрольная сумма не является подписью или защитой от отката.
   Полезная нагрузка имеет ровно 128 байт: `0..4` — `BMIG`, `4` — версия
   `1`, `5` — стадия (`1` = `Pending`, `2` = `Staged`, `3` = `Committed`),
   `6..8` — нули, `8..12` и `12..16` — схемы `3` и `4`, `16..32` — ID,
   `32..40` — ненулевая длина копии, `40..72` — её SHA-256, `72..80` и
   `80..88` — `device` и ненулевой `inode`, `88..96` — число записей staging,
   `96..128` — SHA-256 снимка. У `Pending` последние 40 байт нулевые; у двух
   следующих стадий хеш обязателен, в том числе SHA-256 пустой строки при
   нулевом числе записей. Нулевой хеш копии запрещён.
2. Отдельная таблица `migration_v3_v4_stage` содержит только подготовленные
   записи одного ID миграции. Ключ состоит из ID миграции, именованного вида
   таблицы из закрытого перечня и исходного ключа с явно кодированной длиной;
   значение содержит исходный SHA-256 и полную проверенную запись v4. Виды
   индексов, замков и данных, которые не требуют перекодирования, также явно
   перечисляются; неизвестный вид или таблица запрещают переход. Порядок
   канонического снимка — лексикографический порядок полных ключей staging;
   дайджест считается по однозначному кодированию длины и содержимого каждого
   ключа и значения. Скрытые, лишние или дублирующиеся записи запрещены.
   Вид таблицы — один байт: `1` authenticators, `2` enrollments,
   `3` enrollment_index, `4` mutations, `5` mutation_scope_lock,
   `6` effect_outbox, `7` idempotency, `8` provider_state, `9` packages,
   `10` template_blobs. Ключ staging: `ID[16] || вид[1] || длина[4] || ключ`;
   значение: `SHA256 исходного значения[32] || длина[4] || новое значение`.
   Длины — беззнаковые 32-битные big-endian. Внешний снимок для хеша содержит
   пары `длина ключа[4] || ключ || длина значения[4] || значение` в указанном
   порядке. Пределы исходных ключей и значений остаются пределами документа 23;
   staging не разрешает данные, для которых нет строгого проверяющего кодека.
3. До `Pending` процесс удерживает общий эксклюзивный lock базы, запрещает
   новые операции, проверяет отсутствие активных и неразрешённых мутаций и
   заново выполняет preflight под тем же lock. Системный владелец создаёт
   отдельную резервную копию без перезаписи существующего файла, фиксирует
   файл и содержащий каталог и проверяет по удержанному FD её размер, дайджест
   и полное открытие/согласованность как снимка v3. Владелец повторно сверяет
   идентичность FD и перед публикацией `Pending`; путь и один хеш не заменяют
   проверку копии. Ошибка создания или проверки не меняет исходную базу.
4. Допуск **обязан** включать доказательство для конкретного установленного
   старого бинарника: в изолированных проверках он не открывает публичный
   сокет и не изменяет базу как при `Pending`, так и при маркере v4. Проверка
   связана с удержанным файлом и его измерением; замена бинарника отменяет
   допуск. Если прежний бинарник игнорирует дополнительную запись `meta` при
   маркере v3, этот механизм запрещён: не записывать `Pending`, не полагаться
   на то, что старая программа «должна» отказать. Новый v3 opener тоже
   отказывает на любых незнакомых или незавершённых стадиях до рабочего открытия.
5. `Pending` фиксируется durable в исходной базе только после успешного
   допуска и копии. Затем записи staging строятся без изменения основных
   таблиц; после полной повторной проверки ключей, зеркал, перекрёстных связей
   и дайджеста в одной durable транзакции фиксируется `Staged`. На любой
   ошибке, потере lock, отзыве допуска или неизвестном результате commit
   обычный запуск запрещён; никаких исходных записей автоматически не удалять.
6. Финальная одна durable write-транзакция повторно проверяет маркер v3,
   `Staged`, ID, содержимое и дайджест staging, отсутствие новых мутаций и
   неизменность lock. Она также проверяет взаимно однозначное соответствие
   каждой staging-записи текущей исходной записи по виду, ключу и исходному
   SHA-256, и отсутствие исходных записей без staging; недостаточно только
   сверить дайджест самой staging-таблицы. Затем она переносит все записи
   в основные таблицы, создаёт
   пустую `provider_installations`, ставит маркер v4 и `Committed`. Ни одна
   установка старого пакета автоматически не становится `ACTIVE`. Успех
   сообщается только после подтверждённого commit; при потере подтверждения
   читается новый снимок, а не повторяется переключение вслепую.
7. Обычный v4 opener принимает только однозначный `Committed` с маркером v4
   и проверенными ID/версиями. `Pending`/`Staged`, несогласованный маркер,
   повреждённая запись, незнакомый ключ `meta` и невозможность проверить
   состояние — отказ до публикации сокета. После подтверждённого `Committed`
   cleanup staging допускается отдельной транзакцией; журнал сохраняется.
   Удаление backup или откат копии не выполняются автоматически. Если обычный
   opener ещё не умеет проверять `Committed`, миграцию не включать.
8. При обнаружении `Pending` или `Staged` служба не продолжает миграцию и не
   выдаёт полномочия автоматически. Отдельная административная команда должна
   получить эксклюзивный lock, повторно проверить журнал, FD и целостность
   копии, установить фактическую устойчивую фазу и выполнить явно выбранное
   восстановление. Нельзя считать восстановленную копию доказательством
   актуальности ключей, полномочий или состояния оборудования.

#### Проверки перед включением

Требуются тесты полного цикла со всеми поддерживаемыми таблицами и конфликтами
идентичности, повреждения и усечения журнала/staging/backup, каждой стадии и
каждого окна durable commit, потери подтверждения, конкурентного запуска,
подмены файла копии или старого бинарника и запрета автоматической выдачи
установки. Обязательны реальные прогоны сохранённого v3-бинарника и испытание
отказа питания на целевой ФС; фикстуры и unit-тесты их не заменяют.
