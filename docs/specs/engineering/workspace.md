# Исходный код и границы workspace

**Предложенная структура;** все пути ориентировочны. Наличие директории не означает необходимость отдельного процесса.

```text
carosign/
  README.md
  docs/
  protocols/
    client/v1/
    provider/v1/
  crates/
    carosign-core/
    carosign-provider-api/
    carosign-runtime/
    carosign-registry/
    carosign-ipc/
    carosign-evidence/
  platform/
    linux/
      carosignd/
      carosignctl/
      pam/
      dbus-fprintd-adapter/
      hardware-broker/
  providers/
    libfprint/
    gxfp5130/
    goodix-qsee/
    nord4-qtee/
  sdk/
    rust/
    c/
    cpp/
    go/
    python/
  conformance/
  packaging/
```

## Принципы зависимостей

- `carosign-core` зависит только от доменных типов/стандартной библиотеки и независимых crates; никаких Linux syscalls, Uzel, QSEE, Tokio requirement.
- `carosign-provider-api` — семантические контракты и wire bindings, **не Rust trait ABI** для сторонних providers.
- `carosign-runtime` реализует планировщик, journal coordination, manager, authorization/evidence ports.
- `carosign-ipc` предоставляет server/client transport для Linux; адаптер Veles вне основного workspace.
- `providers/*` могут использовать свои языки и toolchains; их наличие в monorepo — удобство разработки, не dependency для установки.
- `sdk` опционален; conformance работает и без него.

## SDK

Generated Protobuf code + helpers для handshake/framing/correlation/operation state/error mapping, FD transfer на Linux. Для C/C++ abstractions не должны требовать Rust library. Python/Go SDK допустимы и должны запускаться в тех же sandbox профилях. Тестовые vectors и схемы — единый источник совместимости.

## Исходники спецификации

Будущие `.proto` — нормативный формат для двух межпроцессных границ `client` и `provider`; текущее обсуждение в `.md` содержит sketch, не полностью замороженную wire schema. Internal ports можно реализовать Rust traits и переводить в IPC только при необходимости.

## Scope контроля unsafe

FFI/kernel ioctls/SCM_RIGHTS/GObject bindings будут содержать scoped unsafe на уровне адаптеров. Использование `unsafe` в provider C/C++ не изменяет контракт или доверие к нему; процессные границы, параметры сообщений и ресурсы валидируются Runtime/Broker.

## Детализированная структура v0.5

Предыдущая структура — укрупнённая. Актуальная проектная детализация crate/module: [rust/README.md](rust/README.md). Для внутреннего доменного автомата выделен `carosign-engine`; `carosign-protocol` содержит generated wire types; Linux binary/transport, provider SDK и persistence согласованы отдельными документами. Нет обязательной зависимости Rust Runtime от конкретных providers.
