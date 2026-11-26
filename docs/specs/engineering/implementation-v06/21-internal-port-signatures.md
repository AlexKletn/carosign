# 21. Exact internal port contracts and ownership

These **internal Rust** signatures are implementation baseline, not externally stable Rust ABI. Conceptual aliases (`BoxFuture`) must be implemented once in `runtime::ports`/`evidence` and re-exported narrowly; they are not serialization formats. Only controlled versions of types from [03](03-domain-types.md) may cross between crates.

```rust
use std::{future::Future, pin::Pin};
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait IdSource: Send + Sync {
    fn new_operation_id(&self) -> Result<OperationId, IdUnavailable>;
    fn new_mutation_id(&self) -> Result<MutationId, IdUnavailable>;
    fn new_command_id(&self) -> Result<CommandId, IdUnavailable>;
}
pub trait TimeSource: Send + Sync {
    fn now(&self) -> BootInstant;
    fn boot_id(&self) -> BootId;
}
pub trait IdentityAuthority: Send + Sync {
    fn resolve<'a>(&'a self, caller: &'a CallerContext,
        selector: &'a SubjectSelector)
        -> BoxFuture<'a, Result<SubjectRef, AuthorityError>>;
}
pub trait PolicyAuthority: Send + Sync {
    fn authorize<'a>(&'a self, request: AuthorizationRequest<'a>)
        -> BoxFuture<'a, Result<ValidatedGrant, AuthorityError>>;
    fn revalidate<'a>(&'a self, grant: &'a ValidatedGrant,
        boundary: AuthorizationBoundary)
        -> BoxFuture<'a, Result<(), AuthorityError>>;
    fn watch_revocation(&self, grant: &ValidatedGrant) -> RevocationHandle;
}
```

### Resource and provider ports

```rust
pub trait ResourcePort: Send + Sync {
    fn acquire<'a>(&'a self, set: &'a ResourceSet,
        owner: &'a WorkerIdentity)
        -> BoxFuture<'a, Result<ResourceLease, ResourceError>>;
    // Only approved bindings can be delegated. The opaque result is
    // consumed exactly once by the platform's BindAuthenticator adapter.
    fn delegate<'a>(&'a self, lease: &'a ResourceLease,
        approved: &'a [ApprovedResourceBinding])
        -> BoxFuture<'a, Result<DelegatedResources, ResourceError>>;
    fn quiesce<'a>(&'a self, lease: &'a ResourceLease)
        -> BoxFuture<'a, Result<QuiesceAssessment, ResourceError>>;
    fn release<'a>(&'a self, lease: ResourceLease)
        -> BoxFuture<'a, Result<(), ResourceError>>;
}
pub trait ProviderExecutionPort: Send + Sync {
    fn command<'a>(&'a self, session: &'a ProviderSessionRef,
        command: ProviderCommand)
        -> BoxFuture<'a, Result<CommandDispatchReceipt, ProviderError>>;
    fn request_cancel<'a>(&'a self, session: &'a ProviderSessionRef,
        operation: OperationId)
        -> BoxFuture<'a, Result<CancelDispatchReceipt, ProviderError>>;
    fn subscribe<'a>(&'a self, session: &'a ProviderSessionRef)
        -> BoxFuture<'a, Result<ProviderEventSubscription, ProviderError>>;
}
```

`command` is a single send/ack boundary, **not** a guarantee hardware executed. `ProviderEventSubscription` is a bounded receiver/stream, owned by the ProviderSession actor, never consumed by arbitrary caller concurrently. `ResourceLease` constructor private, broker-issued. `ProviderCommand::Mutate` can be created **only** using an opaque `ArmedMutationTicket` emitted from durable repository arm; otherwise impossible at public call-site.

### Persistence: object-safe high-level API

Instead of exposing arbitrary generic transaction closures to Runtime, persistence exports **specific atomic methods**. redb transaction internals remain private.

```rust
pub trait MutationRepository: Send + Sync {
    fn prepare<'a>(&'a self, input: PrepareMutation)
        -> BoxFuture<'a, Result<PreparedMutation, StorageError>>;
    fn arm<'a>(&'a self, input: ArmMutation)
        -> BoxFuture<'a, Result<ArmedMutationTicket, StorageError>>;
    fn record_observation<'a>(&'a self, input: MutationObservation)
        -> BoxFuture<'a, Result<(), StorageError>>;
    fn commit_registry<'a>(&'a self, input: CommitEnrollment)
        -> BoxFuture<'a, Result<RegistryCommit, StorageError>>;
    fn finalize<'a>(&'a self, mutation: MutationId)
        -> BoxFuture<'a, Result<(), StorageError>>;
    fn mark_uncertain<'a>(&'a self, mutation: MutationId)
        -> BoxFuture<'a, Result<(), StorageError>>;
    fn scan_incomplete<'a>(&'a self)
        -> BoxFuture<'a, Result<Vec<MutationRecord>, StorageError>>;
}
```

`PreparedMutation`/`ArmedMutationTicket` constructors private. `arm` must perform checked phase update and durable outbox insert in same transaction, and return ticket **only after durable commit**. Since backend methods may be async for worker scheduling, the internal redb transaction is synchronous and never spans await. Runtime must not manually change tables or call low-level SQL directly.

### Evidence and UI ports

```rust
pub trait EvidencePort: Send + Sync {
    fn evaluate<'a>(&'a self, material: EvidenceMaterialRef,
        context: &'a EvidenceContext, requirements: &'a EvidenceRequirements)
        -> BoxFuture<'a, Result<EvidenceAssessment, EvidenceFailure>>;
    fn invalidate<'a>(&'a self, handle: EvidenceMaterialRef)
        -> BoxFuture<'a, ()>;
}
pub trait InteractionPort: Send + Sync {
    fn prepare<'a>(&'a self, request: InteractionRequest)
        -> BoxFuture<'a, Result<InteractionLease, InteractionError>>;
    fn quiesce<'a>(&'a self, lease: InteractionLease)
        -> BoxFuture<'a, Result<QuiesceAssessment, InteractionError>>;
}
```

UI progress transport is separate from trusted physical interaction; not every device needs `InteractionLease`. No `async_trait` dependency imposed on C providers — these are Rust-only in-process ports.

## Construction / lifecycle invariants

- `RuntimeBuilder::build` fails if any required port absent or persistence schema invalid.
- `OperationActor` is sole writer for its operation; `OwnerWatch` is nonblocking signal into actor, not direct state mutation.
- `MetadataStore` internal `redb` handler owns all write transactions. No DB handle exposed to provider workers.
- All ports return typed errors and have bounded deadlines at caller; provider may not hang global runtime.
- `EvidenceAssessment` and `ValidatedGrant` constructors restricted to verified adapter implementations; ordinary untrusted wire conversion cannot instantiate them.

## Non-final implementation details

Exact struct field visibility and module-local concrete Tokio channel type may be optimized without public semantic changes. If those adjustments would change cancellation ordering, durability, wire, trust or owner boundaries, they are architecture changes requiring ADR.

### Resource delegation refinement v0.6.1

`ResourcePort::acquire` obtains a lease on the **entire** resource set. Before `BindAuthenticator`, an internal broker adapter executes `Delegate(lease, approved_role_bindings)` to produce an exact list of broker-issued `DelegatedHandle { resource_id, role, fd }`; this is an **internal type** and is consumed once when sending the sole authorized `BindAuthenticator` with `SCM_RIGHTS`. The authoritative map is retained in broker state for validation. `FD_ROLE_MEDIATED_SERVICE` passes a connected socket whose messages use `carosign.broker.v1` and whose authority is checked on each call. No provider can construct this trusted handle by deserializing a `FdSlot`. Broker method handlers and their argument validators are installed by trusted system policy; unknown commands fail closed. Do not invent a universal `invoke_any_ioctl`. See [09](09-device-and-broker.md), [25](25-v061-conflict-resolutions.md).

`ApprovedResourceBinding { resource_id: ResourceId, role: ResourceFdRole }` is issued by the trusted resource-policy resolver and cannot be built from worker manifest claims alone. `DelegatedResources` is a single-use, RAII-owned platform handle batch with its exact broker-validated `(lease, worker, authenticator, epoch, resource_id, role)` associations; for Linux its adapter transports approved FDs in `BindAuthenticator` and must close remaining copies on error. `ResourcePort` exposes **no** arbitrary ioctl/TA method. On Veles, equivalent platform capabilities may use non-FD handles; Core and Engine do not depend on Linux `OwnedFd`. The Linux binding adapter is responsible for `FdSlot` order/count and `SCM_RIGHTS` validation.

### Metadata effects fixed for v0.6.2

`MutationRepository::commit_registry(input: CommitEnrollment)` is retained as an umbrella internal method **only if** `CommitEnrollment` is a closed, validated enum with exactly `ApplyDesiredState(VerifiedState)` and `RecordVerifiedNoChange(VerifiedState)` alternatives, dispatched to distinct atomic redb transactions (never an optional/unvalidated bool). `RecordVerifiedNoChange` updates mutation/outbox state but does not mutate `enrollments` or `enrollment_index`. `finalize` requires verified safe cleanup, commits both `MUTATION_COMPLETED` and explicit terminal outcome in the same transaction as scope-lock removal. Any commit-receipt failure after arm produces reconciliation, not success. See [27](27-v062-engine-conflict-resolutions.md).

### v0.6.4 — internal port restrictions

`ProviderExecutionPort::command` is the **only** generic provider invocation boundary for verification, enrollment and removal. No `Matcher`, `Capture`, `TemplateParser` or scoring abstraction is added to Runtime/Core/Engine. Verified scope-resolved bindings are constructed by Registry/Authorization before the `StartVerification` command. A provider-worker may retrieve optional encrypted provider blobs only through a separate scoped vault/broker port, with no central matching functionality; this port's wire/staging remains a STOP/ADR if needed. See [29](29-v064-provider-owned-matching.md).

### v0.6.5 internal trusted extensions

`ProviderCommand` gains a non-persistently-mutating `PrepareEnrollmentChallenge` variant for enrollment actors only. Its validated response is `ChallengePrepared(ValidatedChallenge)`; this cannot create an `ArmedMutationTicket`, authorize HAT or indicate an enrollment completed. The effect runner correlates `(operation, mutation, command, worker_instance, epoch)` and proves the profile's pre-enroll side-effect classification before permitting dispatch. Add the `EnrollmentChallengePort`, `EnrollmentCredentialAuthority`, and `EnrollmentAuthorizationSink` trusted interfaces in [31](31-v065-enrollment-authorization.md). They are separate from `PolicyAuthority` manage grant and EvidenceVerifier.

`ProviderExecutionPort::command` remains the generic provider control entrypoint (including Prepare, Verify, Enroll, Remove); it **does not** expose raw HAT or bulk-byte buffers. Bulk hardware memory is a separate approved `carosign.broker.v1` handler feature with sealed memfd (32). `MutationRepository::prepare` writes exact `MutationRecord.plan` schema v2 atomically with mutation lock and idempotency. No provider invokes `MetadataStore` or owns this plan. A trusted `ReconciliationVerifier` gets both decoded immutable plan and independent physical witness; it never treats Registry data as sufficient hardware truth (33).

### v0.6.6 — command projection authority

Add privileged internal `EnrollmentCommandBuilder` executed in Runtime before `arm`/send, obtaining the exact Registry binding and validating it against the immutable v3 plan under the active scope lock. `ProviderExecutionPort::command` receives the resulting typed `CommandPlan` sealed by the `ArmedMutationTicket`; it MUST NOT accept raw unvalidated nullable `old`/`desired` enrollment arguments. Builders do not live in Core, Engine or the provider process. See [34](34-v066-enrollment-command-projection.md).

## Отказ источника идентификаторов

<a id="fallible-id-source"></a>

### Отказ источника идентификаторов

Все методы `IdSource` возвращают типизированный отказ `IdUnavailable`.
Linux использует ограниченный неблокирующий `getrandom`, дочитывает короткие ответы и ограничивает повторы прерываний и нулевых результатов. Частично заполненный буфер отбрасывается. Идентификаторы сохраняют прежние типы и wire-кодирование.

Отказ до публикации операции даёт `UNAVAILABLE` без записи идемпотентности, места соединения, выдачи ресурсов или команды. При отказе выдачи ID соединения закрывается это соединение. Отказ после начала передаётся машине состояний. Выданные ресурсы удерживаются до подтверждённой уборки, а неопределённая отправка или устойчивый `DispatchArmed` требуют сверки без повтора команды.

#### Точный договор

1. Все методы `IdSource` возвращают `Result<соответствующий Id, IdUnavailable>`.
   Метод остаётся у прежнего владельца: операция, изменение, команда, работник,
   выдача, доказательство, ключ и соединение не становятся взаимозаменяемыми.
   Типизированные 16-байтовые ненулевые идентификаторы и wire не меняются.
2. Linux-адаптер использует ограниченный неблокирующий `getrandom`, дочитывает
   короткий ответ и ограничивает число прерываний и нулевых результатов. При
   отказе возвращает `IdUnavailable`; частично заполненный буфер не выдаётся.
   Повтор запроса клиентом не является разрешением повторить аппаратную команду.
3. Неуспех выдачи ID до создания или публикации операции возвращает
   `UNAVAILABLE`. Операция, запись идемпотентности, место соединения, выдача и
   команда не создаются. Если ID относится к соединению, соединение закрывается;
   другие клиенты и служба продолжают работать.
4. Если ошибка обнаружена после начала операции, владелец передаёт типизированный
   отказ в существующую машину состояний. Выданные ресурсы удерживаются до
   подтверждённой уборки. После `DispatchArmed` или неопределённой отправки
   повторной команды нет; исход изменения остаётся в предусмотренной журналом
   области сверки. Ошибка источника идентификаторов не подтверждает, что
   команда не дошла или устройство находится в покое.
5. Все реализации и тестовые источники переводятся на новый договор одновременно;
   временного fallback на старый непадающий trait или `catch_unwind` нет.
   Настоящий источник выдаётся корнем сборки; клиентские байты не становятся
   платформенным ID. Публичному клиенту возвращаются существующие коды ошибки,
   новый wire-тег или новый код протокола не добавляется.

#### Проверки до подключения

Покрыть отказ каждого вида ID и повторные нулевые значения. На настоящем
`SOCK_SEQPACKET` проверить отказ одному клиенту и работу следующего; до допуска
провайдер не вызывается. Отдельно проверить отказ до отправки, после выдачи
ресурсов и после `DispatchArmed`: отсутствие повтора, удержание ресурсов и
сохранение необходимого состояния сверки. Проверить конкурентные операции,
переполнение и успешный случай с источником ядра. Испытания с подставным
источником не доказывают работу оборудования.
