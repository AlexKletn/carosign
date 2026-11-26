# 02. Точное дерево crates и внутренних модулей

Ниже *целевой исходный layout* v1; новые публичные crates без ADR не добавлять. `mod.rs` допускается только когда явно отмечен каталог; остальные `.rs` — отдельные модули. Внешние аппаратные провайдеры — **вне основного Cargo workspace**.

## Принципы организации кода

Эти требования обязательны для платформы и дополняют установленные ниже
границы и правила изменения контрактов. Принципы Clean Code и Clean Architecture
применяются с учётом инвариантов этой спецификации, а не вместо них.

- **Одна смысловая сущность на файл.** Файл владеет одной основной сущностью
  или одной тесно связанной обязанностью; имя файла называет её по смыслу.
  Реализация сущности, её закрытые вспомогательные типы и локальные испытания
  могут находиться рядом. Несвязанные сущности и обязанности выделяются в
  отдельные файлы и модули. Не дробить код на файлы ради механического счёта:
  итоговая структура должна оставаться связной и понятной.
- **Декомпозиция крупных файлов.** Крупный файл рассматривается на предмет
  смешения обязанностей и при обнаружении разделяется по смыслу и владельцам.
  Машина состояний, её переходы и необходимые для их понимания инварианты
  должны оставаться явно связаны. Перенос кода сам по себе не меняет поведение,
  порядок эффектов, атомарность, wire или формат хранения.
- **Имена и границы.** Имена типов, файлов и функций выражают доменные понятия
  и действие; не использовать порядковый номер версии как замену назначению.
  Номера допустимы в договорах версионирования, конкретных форматах и шагах
  миграции, когда они действительно различают совместимость.
- **Зависимости направлены внутрь.** Домен и правила операций не зависят от
  UI, протокола, ОС, конкретной базы или аппаратного провайдера; внешние
  механизмы подключаются через принадлежащие внутреннему слою порты. Состав
  конкретных реализаций остаётся в точке сборки. Указанный ниже DAG и запреты
  на аппаратную семантику в общем ядре сохраняют силу.
- **Ответственность и простота.** Модуль делает одну согласованную работу;
  проверки, политика, хранение, транспорт и представление не смешиваются.
  Повторяющееся знание выделяется по общему владельцу, а не по внешнему
  сходству кода. Не добавлять абстракции и зависимости без подтверждённой
  потребности; явные типы и состояния предпочтительнее скрытых соглашений.
- **Ошибки и проверяемость.** Ошибки и неопределённые исходы сохраняют свой
  смысл, не превращаются молча в успех. Модули проектируются для испытаний
  через принадлежащие им границы; проверки покрывают поведение и инварианты,
  включая отмену, восстановление и сбои на соответствующих границах.
- **Рефакторинг и совместимость.** До декомпозиции устанавливаются текущие
  договоры и проверки; после неё проверяются поведение, зависимости и
  отсутствие потери данных. Публичные интерфейсы и нормативное дерево меняются
  по правилам [01](01-decisions-and-change-control.md); изменения формата и
  совместимости хранилища — по [10](10-persistence-and-recovery.md).

```text
carosign/
  Cargo.toml
  Cargo.lock
  contracts/proto/carosign/{common,provider,client}/v1/*.proto
  crates/
    core/
      carosign-core/src/
        lib.rs ids.rs identity.rs modality.rs device.rs provider.rs
        capabilities.rs enrollment.rs resource.rs authorization.rs
        assurance.rs evidence.rs error.rs limits.rs
        operation/{mod.rs,context.rs,verification.rs,mutation.rs,cancellation.rs}
      carosign-engine/src/
        lib.rs input.rs effect.rs decision.rs transition.rs invariant.rs
        machines/{mod.rs,verification.rs,enrollment.rs,removal.rs,worker.rs,device.rs}
        recovery.rs replay.rs
      carosign-evidence/src/
        lib.rs material.rs verifier.rs assessment.rs requirements.rs
        recipient.rs routing.rs policy.rs error.rs
    runtime/
      carosign-runtime/src/
        lib.rs builder.rs handle.rs ports.rs services.rs
        operation/{mod.rs,actor.rs,registry.rs,owner.rs,observer.rs,events.rs}
        flow/{mod.rs,verify.rs,enroll.rs,remove.rs,cancel.rs,finalize.rs}
        scheduler/{mod.rs,resource_graph.rs,queue.rs,lease.rs}
        provider/{mod.rs,inventory.rs,resolver.rs,manager.rs,supervisor.rs,session.rs}
        authority/{mod.rs,caller.rs,subject.rs,grant.rs,revocation.rs}
        recovery/{mod.rs,startup.rs,reconcile.rs,blocked_scope.rs}
        effect_runner.rs dispatcher.rs time.rs diagnostics.rs audit.rs
      carosign-persistence/src/
        lib.rs model.rs store.rs transaction.rs registry.rs journal.rs
        outbox.rs bindings.rs recovery.rs migration.rs integrity.rs
        redb/{mod.rs,tables.rs,codec.rs,tx.rs}
        template/{mod.rs,store.rs,protection.rs,key_provider.rs}
      carosign-adapters/src/
        lib.rs boot.rs clock.rs evidence.rs gateway.rs identity.rs mutation.rs
        policy.rs projection.rs provider.rs recovery.rs resource.rs
    protocol/
      carosign-protocol/src/
        lib.rs generated/{mod.rs,common.rs,provider.rs,client.rs}
        limits.rs validation.rs version.rs
      carosign-provider-sdk/src/
        lib.rs boot.rs handshake.rs dispatch.rs command.rs events.rs
        cancellation.rs context.rs wire.rs
    linux/
      carosign-linux/src/
        lib.rs inventory.rs identity.rs policy.rs time.rs paths.rs
        resources/{mod.rs,broker_client.rs,claim.rs,fd.rs}
        process/{mod.rs,spawn.rs,pidfd.rs,sandbox.rs,verified_exec.rs}
      carosign-linux-ipc/src/
        lib.rs frame.rs credentials.rs fd_slots.rs codec.rs
        client/{mod.rs,accept.rs,session.rs,dispatch.rs,observe.rs}
        provider/{mod.rs,connection.rs,handshake.rs,bridge.rs}
        mapping/{mod.rs,client.rs,provider.rs}
      carosignd/src/{main.rs,config.rs,bootstrap.rs,assemble.rs,server.rs,shutdown.rs}
      carosign-broker/src/{main.rs,policy.rs,server.rs,resources.rs,fd_policy.rs,recovery.rs}
    clients/
      carosignctl/src/{main.rs,commands.rs,client.rs,output.rs}
      carosign-fprintd-compat/src/{main.rs,dbus.rs,translate.rs,ownership.rs}
      pam-carosign/src/{lib.rs,pam_ffi.rs,conversation.rs,result.rs,client.rs}
    testing/
      carosign-testkit/src/
        lib.rs clock.rs authority.rs resource.rs provider.rs store.rs
        faults.rs trace.rs fixtures.rs
      carosign-mock-provider/  # самостоятельный workspace, испытательный провайдер
    tools/
      carosign-fence-inspect/  # самостоятельный workspace, утилита диагностики
  conformance/
  docs/
```

## Модули по ownership (не нарушать)

| Модуль | Вход | Выход | Что категорически не делает |
|---|---|---|---|
| `core::ids` | 16 байт от trustable generator | typed ID | Не генерирует права, не делает I/O |
| `core::identity` | scoped identity | `SubjectRef` | Не читает `/etc/passwd` |
| `core::enrollment` | reference + metadata | immutable domain records | Не хранит bytes шаблона |
| `engine::transition` | state + verified observation | `Decision` | Нет await, FFI, файлов, времени |
| `persistence::journal` | mutation intent/phase | durable receipt | Не отправляет TA commands |
| `persistence::redb::tx` | transaction closure | commit result | Не работает с внешними async calls |
| `runtime::operation::actor` | authorized op + mailbox | effects/outcome | Нет Goodix/libfprint типов |
| `runtime::provider::resolver` | inventory + catalog | binding decision | Не запускает probe без политики |
| `runtime::provider::supervisor` | approved executable | checked process/channel | Не выдаёт hardware assurance |
| `runtime::scheduler` | resource claims | reservation/lease | Не знает credential policy |
| `evidence::verifier` | material + context + trusted inputs | `Assessment` | Не принимает декларацию worker за гарантию |
| `linux-ipc::mapping` | protobuf wire | validated domain request | Не создаёт trusted `CallerContext` из тела пакета |
| `carosignd::assemble` | configured concrete ports | `RuntimeBuilder` | Не реализует доменную state machine |

## DAG

`core` — корень; `engine → core`; `persistence → core`; `evidence → core`; `runtime → core + engine + persistence + evidence`; `protocol` — independent wire; `provider-sdk → protocol`; `linux → core`; `linux-ipc → protocol + core` (mapping и transport); бинарники собирают зависимости. Важно: `runtime` получает provider transport через `ProviderExecutionPort`, а не импортирует `carosign-linux-ipc`, иначе возникнет platform-specific coupling.

## Visibility

`lib.rs` экспортирует только документированные фасады и модели. Все `pub(crate)` types не часть ABI. Предпочитать `pub(crate)` по умолчанию. `pub use` только для явно поддерживаемых типов, отсутствие wildcard exports. `#[forbid(unsafe_code)]` для `core`,`engine`,`evidence` (кроме возможных явно разрешённых крипто-FFI в инфраструктуре, не внутри evidence).

## Ownership зависимости

Конкретный `StateStore`/`EvidenceVerifier` и `AuthorityPort` создаются composition root; цикл зависимостей отсутствует. Trait boundary внутри процесса не публикуется C/Rust ABI: это только Rust interfaces. Внешние worker — only wire contract.
