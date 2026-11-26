# `carosign-provider-libfprint`: inbound compatibility

## Назначение

Отдельно устанавливаемый provider-worker, использующий публичный C API libfprint. Это позволяет поддержать существующие совместимые устройства без пересборки Core и без форка библиотеки. Provider может быть написан на C/GLib либо другом языке через FFI — wire-контракт остаётся language-neutral.

## Основные отображения

| `carosign.provider` | libfprint (концептуальное соответствие) |
|---|---|
| `Discover` | `FpContext` / `fp_context_get_devices()` |
| `OpenDevice` | `fp_device_open()` |
| `Describe` | `fp_device_get_features()` |
| `Enroll` | `fp_device_enroll()` |
| `Verify` | `fp_device_verify()` |
| `Identify` (optional) | `fp_device_identify()` |
| `Enumerate` (optional) | `fp_device_list_prints()` |
| `Remove` | `fp_device_delete_print()` |
| `Cancel` | `GCancellable` (асинхронная семантика) |
| `Close` | `fp_device_close()` |

Реальная доступность операции зависит от device feature set и версии libfprint. Не заявлять `Enumerate/Delete` на устройстве без соответствующей функциональности.

## Worker event loop

Долгоживущий `GMainContext`; control IPC доступен во время асинхронной операции libfprint. `g_cancellable_cancel()` не гарантирует моментального освобождения устройства. Worker сообщает финальный state только после callback/подтверждённого quiesce.

## FpPrint и хранение

Использовать libfprint serialization/deserialization там, где разрешено. Сохранять template format/schema, identity устройства, provider version, compatibility descriptors; применять совместимость `FpPrint` с конкретным устройством. Software-stored templates шифруются общим защищённым storage port, не идут в public IPC. Нет предположения, что все устройства экспортируют одинаковые данные.

## Совместимость fprintd — другой адаптер

`libfprint bridge` — **входящая** поддержка hardware. `fprintd-compatible D-Bus` — **исходящая** поддержка Linux clients. Не склеивать их и не запускать fprintd одновременно с прямым libfprint-провайдером на одном сенсоре.

## Миграция

Наличие уже зарегистрированных fprintd fingerprints не означает право молча импортировать или удалять их. Нужна явная процедура, device compatibility, проверка subject ownership и policy. Невозможный безопасный импорт → требование повторной регистрации, а не silent reset.

## Ограничения безопасности

Программный или Match-on-Chip результат libfprint не автоматически соответствует аппаратному proof. Уровень доверия оценивается отдельно. Если существующий драйвер предоставляет только bool match, аппаратную подтверждаемость нельзя достроить в bridge.

### v0.6.4 — bridge does matching only through libfprint

The C bridge never asks Runtime to compare or decode `FpPrint`. It must restrict verification to the supplied authorized enrollment binding set using libfprint's supported verification/identification semantics; if a selected driver cannot enforce the claimed-subject scope, reject instead of publishing a cross-user match. The driver may match internally on device or in software without changing the platform contract. See [29](../engineering/implementation-v06/29-v064-provider-owned-matching.md).
