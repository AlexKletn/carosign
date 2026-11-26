# Реестр архитектурных решений (ADR)

**Состояния:** `Direction` — согласованное направление; `Proposed` — конкретный способ реализации обсуждается; `Open` — решение отсутствует.

| ADR | Статус | Решение | Причина |
|---|---|---|---|
| 001 | Direction | Core независим от Linux/Veles | переносимость и повторное использование |
| 002 | Direction | Hardware providers — самостоятельные пакеты | отсутствие зависимости поддерживаемых устройств от Core |
| 003 | Direction | Providers используют language-neutral process protocol | не заставлять писать на Rust |
| 004 | Direction | Rust — язык основной реализации, не public ABI | безопасность и управляемость Core |
| 005 | Proposed | Linux seqpacket + Protobuf | message boundaries, codegen, FD passing |
| 006 | Direction | Worker запускается/наблюдается платформой | отказоустойчивость, ограничение привилегий |
| 007 | Proposed | Manifest/Profile split | отделение package identity от hardware compatibility |
| 008 | Direction | `Matched` != system authentication | явная trust boundary |
| 009 | Direction | Unknown hardware assurance не считается Verified | предотвращение ложного класса безопасности |
| 010 | Direction | Enrollment/remove требуют отдельного grant | защита от захвата сессии |
| 011 | Direction | Durable mutation intent + reconciliation | невозможность единого ACID с TA |
| 012 | Direction | Удаление provider не удаляет enrollments | сохранность пользовательских данных |
| 013 | Direction | libfprint — входящий provider bridge | использование существующей hardware support |
| 014 | Direction | fprintd compatibility — отдельный исходящий adapter | разделение управления железом и клиентским API |
| 015 | Direction | Veles-specific API в внешнем adapter | не делать Veles зависимостью Core |
| 016 | Proposed | Один worker на независимое hardware instance по умолчанию | изоляция и управляемость |
| 017 | Proposed | Отдельные Registry, TemplateStorage, MutationJournal | корректный ownership |
| 018 | Open | Нормативные `.proto` поля и features | до финального Provider Protocol v1 |
| 019 | Open | Схема persisted journal и защита rollback | требуется выбор storage/secure anchors |
| 020 | Open | Confidence model для dynamic device selection | требует device-specific данных |

## Как менять решение

Изменения статуса оформлять отдельным ADR с исходным мотивом, альтернативами и migration impact. Уточнение wire semantics, меняющее безопасность, не маскировать как additive minor feature.

## Предложения для ADR v0.5 (ещё не утверждены)

| ID | Статус | Предложение |
|---|---|---|
| 021 | Proposed | Выделить `carosign-engine` как pure transition/effect state machines |
| 022 | Proposed | `Registry` и `MutationJournal` логически отдельны, но в одном transactional metadata store |
| 023 | Proposed | `carosign-protocol` содержит только generated wire types, mapping в adapters |
| 024 | Proposed | Runtime фасад не выдаёт наружу executor-specific types |
| 025 | Proposed | Каждый provider хранит зависимую от него систему модулей в отдельном workspace/release |
| 026 | Proposed | Первый `carosign-persistence` backend — redb при подтверждении durability requirements |

См. [детализацию Rust](rust/README.md).


## Implementation baseline v0.6 (proposed as binding instructions for coding agent)

| ADR | Status | Decision |
|---|---|---|
| 027 | Baseline v0.6 | External schema defined in `contracts/proto/carosign/{common,provider,client}/v1`, not Rust traits |
| 028 | Baseline v0.6 | Persist `DispatchArmed` and effect outbox before any mutating wire send; no blind retry |
| 029 | Baseline v0.6 | redb metadata DB with single transactional journal/registry/outbox; storage schema v1 |
| 030 | Baseline v0.6 | Runtime uses internal Tokio but Core/Engine and Provider Protocol are executor independent |
| 031 | Baseline v0.6 | Provider packages independently installable with private supervisor-owned worker socket |
| 032 | Baseline v0.6 | Numeric/encoding defaults, typed error semantics, worker phases, port ownership in v0.6 specification |
| 033 | Baseline v0.6 | Provider declared feature != trusted assurance; verifier required |
| 034 | Baseline v0.6 | Auth actor/owner/observer independent; cancel request != hardware quiesce |

These are **locked for implementation instructions**, not a claim of externally published ABI or hardware qualification. Change through explicit ADR accepted by project owner.
