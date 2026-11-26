# 26. Матрица внутренних контрактов: входы, выходы, владение

**Статус:** предлагаемые контракты для проектирования `carosign-runtime` и сопутствующих crates. Не являются стабильным Rust ABI и не заменяют будущие нормативные `.proto`.

## Контракты между модулями

| Владелец / модуль | Вход | Выход | Инвариант |
|---|---|---|---|
| `linux-ipc::server::authz_context` | kernel peer creds + connection | `CallerContext` | создан только из доверенного источника; строка `user` не имеет полномочий |
| `runtime::authz::subject_resolver` | caller, selector | `SubjectRef` | пространство идентичностей определяет authority |
| `runtime::authz::grant_request` | caller, subject, operation purpose | trusted `ScopedGrant` | результат нельзя сконструировать из присланного GrantId |
| `runtime::operation_registry` | grant, operation request | `OperationTicket`, `OwnerHandle` | один владелец операции; ID не является секретом |
| `engine::transition` | state + event | new state + ordered effects | без I/O и side effects |
| `runtime::effects` | `Effect` | durable observation / provider observation | каждое событие связано с operation и epoch |
| `scheduler::allocator` | canonical `ResourceSet` | reservation / no grant | атомарность набора claims, отсутствие двойного эксклюзивного lease |
| `providers::resolver` | candidate hardware + approved catalog | provider/profile binding | конфликт владельцев разрешается системной политикой |
| `providers::spawn` | selected executable + private IPC | verified worker handle | не принимает user-supplied executable path |
| `providers::handshake` | Hello, profile, instance | `BoundWorkerSession` | версии и worker identity проверены до доступа к железу |
| `provider-sdk::worker` | `Start*`, `Cancel`, `Bind` | progress/outcome | ровно один терминальный исход в исправном соединении |
| `operation_events` | internal observations | sequenced public events | сырой образец/секрет не попадает в публичное API |
| `persistence::journal` | mutation intent / observations | `DurablePrepared`, journal state | Dispatch только после durable intent |
| `persistence::registry` | expected generation + binding | committed generation | optimistic CAS; чужая регистрация не подменяется |
| `recovery::reconcile` | incomplete journal + safe query port | completed/no-change/indeterminate | не повторяет TA mutation из-за обрыва |
| `evidence::verifier` | scoped material + context | verified properties or reject | заявка provider не создаёт аппаратную гарантию |
| `interaction::controller` | operation lease + physical request | scoped interaction lease | при отмене выключение подсветки и quiesce обязательны |
| `linux-ipc::server::subscriptions` | authorized observer + sequence | events/gap/snapshot | observer не владеет самой операцией |
| `pam::result_map` | authorized verification result | PAM code | нет прямого доступа к sensor или secret store |

## Типовой сквозной портовый интерфейс (эскиз)

```rust
pub trait PortSet: Send + Sync {
    type Authority: PolicyAuthority;
    type Resources: ResourcePort;
    type Providers: ProviderExecutionPort;
    type Metadata: StateStore;
    type Proofs: EvidenceVerifier;
    type Clock: TimeSource;
}
```

Это полезная **композиционная** модель, но не нужно обязывать все внешние реализации, включая C-провайдер, реализовывать такие Rust traits. Провайдер подчиняется `carosign.provider.v1`; адаптер Runtime преобразует его wire-команды в `ProviderExecutionPort`.

## Правила конкурентного исполнения

1. **Операция:** один последовательный reducer, состояния `OperationId`, `DeviceEpoch`, `WorkerInstance` проверяются при каждом наблюдении.
2. **Провайдер:** командный канал управления доступен независимо от блокирующих аппаратных функций.
3. **Хранилище:** журнал/Registry транзакции короткие, не пересекаются с `await` и TA-вызовами.
4. **Арбитраж:** не держать глобальную блокировку во время получения FDs, запуска worker или ожидания ответа.
5. **События:** неприоритетный прогресс может теряться с `Gap`, но терминальный результат мутации нельзя потерять бесследно.
6. **Отмена:** отзыв прав не равен освобождению железа; ресурс блокируется до подтверждения безопасности.

## Изоляция ошибок

- Неверный wire пакет: разрыв конкретной сессии; никакого side effect.
- Неверный `worker_instance`/epoch: игнорировать или protocol fault; не менять Registry.
- Policy revoke: не выдавать verification result, прекратить новые действия; если mutation dispatch уже возможен, проверить journal.
- Worker crash: verification fail closed; mutating operation → `NeedsReconciliation`; worker/process restart отдельно от hardware recovery.
- Corrupt persisted metadata: отметить affected scope недоступным и сохранить файлы для восстановления; не создавать ложный пустой Registry.
- Evidence verification failure: `Unverified`/reject, а не fallback к hardware-level claim из manifest.
