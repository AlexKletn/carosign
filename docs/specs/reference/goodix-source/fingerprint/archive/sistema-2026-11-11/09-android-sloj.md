# 09 — Android-слой: QSEECom, ION, свойства

Части Android, которые наш путь **заменяет**, а Rust должен бесшовно
заменить у себя.

## `libQSEEComAPI.so` → `/dev/qseecom`

Стоковый HAL зовёт `QSEECom_start_app`, `QSEECom_send_cmd`,
`QSEECom_send_modified_cmd`, `QSEECom_shutdown_app`. Всё это по сути ioctl
к `/dev/qseecom` (см. [01](01-yadro-shim.md)):
`LOAD_APP`, `APP_LOADED_QUERY`, `SET_MEM_PARAM`, `SEND_CMD`, `SEND_MODFD_CMD`.
Наш шим отвечает так, чтобы сток шёл по штатному пути `LOAD_APP`
(`query_mode=0`) ✅SRC; иначе получал `1002` ✅MSR.

## ION / dma-heap

`libgf_ca.so` выделяет **ion-буфер** под тело (`gf_sec_ion_malloc`, pla
`0x20e0`) и отдаёт **fd** в `SEND_MODFD_CMD`. Шим берёт `dma_buf_get(fd)` и
копирует в tzmem ✅SRC. На нашем пути Android-ION нет — есть `/dev/dma_heap/system`
(замена через `libion`-шим) ✅SRC; Rust ходит туда напрямую
(`DMA_HEAP_IOCTL_ALLOC`, затем `DMA_BUF_IOCTL_SYNC`).

Требования:
* тело живёт в DMA-буфере, размер ≥ объявленной длины;
* адрес тела передаётся **в заголовке**, а не fd;
* данные всегда `DMA_BUF_IOCTL_SYNC` до и после (кэш).

## Свойства Android

`libspinelprop.so` подменяет `__system_property_get` и уведомления HAL, чтобы
`libgf_hal.so` запускался без Android ✅SRC. Для Rust это не нужно: HAL не
вызывается. Но значения, которые HAL оттуда читал, в нашем клиенте задаются
явно (`group`, вид команды).

## Чем это кончилось на практике ✅MSR

| этап | эталон (HAL) | наш Rust |
|---|---|---|
| открытие | `gf_hal_open` → 0 | `DETECT_SENSOR`, `INIT`, `INIT_FINISHED` → 0 |
| модель | грузится HAL | `1111` с весами `DLWeightLightDubaiB16BIT` |
| событие | приходит от ядра | приходит от ядра (после правки порта) |
| распознавание | `auth_ok`, `score=35` | обработка `1016` без ошибок, итог не получен |

⚠️ Ключевое расхождение — не в Android-части, а в том, **кто и когда
регистрирует получателя событий** и **как обрабатывается край**; см.
[10](10-rust-dolzhen.md) и [11](11-otkrytye.md).
