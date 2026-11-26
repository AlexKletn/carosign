# Проверка пакета v0.6.1

Проверено при сборке 10 ноября 2026:

- Исходные 4 `.proto` схемы (`common`, `provider`, `client`, `storage`) и новая схема `broker` скомпилированы `protoc` 3.13.0 с `-I contracts/proto` и экспортом descriptor set; синтаксических ошибок не обнаружено.
- Проверены локальные Markdown-ссылки новых разделов, парность fenced code blocks, наличие исходных файлов из v0.5.
- ZIP проходит `ZipFile.testzip()` — CRC/чтение записей корректны.

**Не выполнялось:** сборка Cargo/Rust-кода (в пакете нет готовой реализации crates), black-box interoperability с C-провайдером, испытания на GXFP/Goodix/Nord4, аварийные испытания питания и доказательства аппаратной безопасности. `.proto` синтаксически корректны, но ABI freeze требует conformance и принятия проектом.

**v0.6.1:** добавлены решения 25/26 и `bio.broker.v1`. `protoc` проверил все 5 схем. Дополнительные проверки ссылок, тестовых инвариантов документации и целостности ZIP отражены в итоговой сборке пакета. Аппаратная Goodix recovery query **не подтверждена**.


## v0.6.3

Проверка генератора: ссылки локальных MD, ZIP CRC и контрольный MANIFEST. Изменения не затрагивают `.proto`, компиляция исходников Rust и conformance не выполнялись. Точный persisted формат pre/desired reconciliation plan остаётся архитектурным блокером, а не фиктивно решённой задачей.

## v0.6.4

Added normative matching-boundary decision 29 and new provider wire fields. Source-level consistency review and ZIP CRC are performed at packaging time. The Protobuf **syntax cannot be claimed verified without an installed protoc-compatible compiler**; a compiler check must be run as the next gate. No Rust code, independent C wire conformance or hardware security proof are claimed.

## v0.6.5 audit / packaging status

Documents 30–33 and reconstructed gap note integrated into normative sources and four changed `.proto` files. Source references, sample provider minors, HAT/broker/bulk/mutation-plan consistency must pass package static checks. Current environment has Python protobuf runtime but **no installed `protoc` or `buf` compiler**; therefore this release MUST NOT claim Protobuf compilation, generated descriptor compatibility or real IPC interoperability. No Rust/hardware tests were executed. Goodix HAT acceptance in archived research ended with secure-object-write failure; this is not enrollment success. Full independent driver/broker acceptance remains explicitly open.

## v0.6.5 residual source integrity

The inherited `reference/**` corpus contains 14 unresolved Markdown links pointing outside the provided archive (including an original host path); leave them as traceable historical evidence. New normative documents and existing non-reference documents have no broken local Markdown links according to packaging-time static link checks. No claim of verified external target validity.

## v0.6.6 validation limitations

B14-01..B14-12 test scenarios added to document 34. v0.6.6 package creation checks hashes, ZIP, document links and static schema field declarations. It does not claim Rust implementation, C/Rust interoperability, Gatekeeper/HAT integration, or any fingerprint-hardware result. Protobuf compiler result must be reported separately from static schema checks.
