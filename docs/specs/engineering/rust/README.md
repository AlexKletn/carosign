> **Актуальный implementation baseline v0.6:** [полная спецификация для исполнителя](../implementation-v06/README.md). Этот раздел v0.5 сохранён как контекст; его эскизы не имеют приоритета перед контрактами v0.6.

# Rust-проекты: детальная декомпозиция v0.5

**Статус:** архитектурный проект, внутренняя спецификация модулей, не реализованный код. Базовые границы и инварианты перенесены из [архитектуры v0.4](../../README.md). Отдельно помечены предложения, которые требуют выбора при реализации. Последняя ревизия: 10 ноября 2026.

## Навигация

| Раздел | Содержание |
|---|---|
| [01](01-workspace.md) | Физическая структура workspace, владение кодом, публикация crates |
| [02](02-dependencies.md) | Допустимый граф зависимостей и границы I/O |
| [03](03-core.md) | `carosign-core`: типы, инварианты, доменные ошибки |
| [04](04-engine.md) | `carosign-engine`: чистые автоматы операций и эффекты |
| [05](05-runtime.md) | `carosign-runtime`: исполнение операций, слоты, события |
| [06](06-provider-manager.md) | Discovery, candidate resolver, supervisor, update |
| [07](07-operations.md) | Verification/enrollment/removal coordinators |
| [08](08-scheduler.md) | Арбитраж ресурсов, DeviceLease и отмена |
| [09](09-provider-protocol.md) | `carosign-protocol` + `carosign-provider-sdk` |
| [10](10-client-protocol.md) | `carosign-linux-ipc`: внешнее API и сессии |
| [11](11-persistence.md) | `carosign-persistence`: Registry, журнал и recovery |
| [12](12-evidence.md) | `carosign-evidence`: proof и assessment |
| [13](13-authority.md) | Authority, Subject, grants, revocation |
| [14](14-linux-resources.md) | Linux resource broker и аппаратные FD |
| [15](15-linux-service.md) | `carosignd`, discovery, config, privilege lifecycle |
| [16](16-system-clients.md) | PAM, CLI, fprintd API, UI |
| [17](17-gxfp5130.md) | Rust provider GXFP5130 по внутренним модулям |
| [18](18-goodix-qsee.md) | Rust provider Goodix QSEE по внутренним модулям |
| [19](19-nord4-qtee.md) | Nord 4, FOD и Veles adapter |
| [20](20-libfprint-bridge.md) | Нерустовый C/GLib provider, ABI границы |
| [21](21-faults-observability.md) | Ошибки, телеметрия и диагностика |
| [22](22-security.md) | Инварианты безопасности и privileged boundaries |
| [23](23-test-strategy.md) | Conformance, property/fault/integration testing |
| [24](24-implementation-sequence.md) | Вертикальные срезы, контрольные решения |
| [25](25-message-flows.md) | Внутренние вызовы и последовательности |
| [26](26-internal-contract-matrix.md) | Матрица входов/выходов и инвариантов модулей |

## Строгие правила чтения

- **Утверждённое направление:** Rust для реализации платформы, но *не* для сторонних провайдеров; Provider Protocol — межпроцессный language-neutral контракт; Veles — адаптер, не зависимость.
- **Проектируемая реализация:** имена Rust-модулей, типы портов, список crates, конкретные зависимости и выбор executor — **предложение v0.5**. Менять можно через ADR с оценкой совместимости.
- **Не выдавать эскизы за API:** сигнатуры Rust в Markdown иллюстрируют семантику. Стандартизован только будущий wire `.proto`, а он пока не заморожен.
- **Соответствие источникам:** описания GXFP, Goodix и Nord 4 учитывают предоставленные справочные файлы; предложение модулей — новая проектная работа, а не описание уже реализованных источников.

## Ключевое уточнение v0.5

`carosign-core` содержит значения/инварианты; `carosign-engine` — чистые переходы; `carosign-runtime` — I/O orchestration. `carosignd` — composition root, а не монолит с доменной логикой. Registry и MutationJournal **логически** раздельны, но могут храниться в **одной транзакционной БД** для согласованной фиксации метаданных и журнала. Хранилище физических шаблонов остаётся отдельным.
