# 01 — Ядро: шим `/dev/qseecom`

Файл: `linux-rom/fingerprint/qseecom-shim/spinel_qseecom.c` (477 строк). ✅SRC
Зачем: стоковый `libQSEEComAPI.so` умеет только ioctl к `/dev/qseecom`; шим
переводит их в наши вызовы `qcom_scm_qseecom_*`.

## Устройство

* misc-устройство `qseecom`, права `0666`, свой пул tzmem:
  `initial 8 МБ`, рост по 8 МБ до 64 МБ, `ON_DEMAND`.
* Глобальная блокировка `qc_lock` на все ioctl.
* На файл — `qc_ctx`: `app_id`, `sb_base`, `sb_len`.
* Параметры модуля: `query_mode` (0 — «не загружено»), `fix_dlina`
  (`0x4FFA8`), `native_guard` (`1`), `skip109`.

## ioctl'ы и что они делают

| ioctl | nr | поведение |
|---|---|---|
| `REGISTER_LISTENER` / `UNREGISTER` / `RECEIVE` / `SEND_RESP` | 1, 2, 5, 6 | заглушка (`rc=0`): службу слушателя держит `spinel_qsee_listener` |
| `SEND_CMD_REQ` | 3 | `qc_send(..., nfd=0)` |
| `SEND_MODFD_CMD_REQ` / `_64` | 4, 35 | `qc_send(..., ifd, 4)` |
| `LOAD_APP_REQ` | 7 | имя без суффикса после точки; id берётся **сначала** из наших `spinel_fp_loaded_app_id` / `spinel_km_app_id`, иначе `qcom_scm_qseecom_app_get_id` |
| `SET_MEM_PARAM_REQ` | 8 | запоминает `sb_base/sb_len`, дальше не используется |
| `UNLOAD`, `PERF_ENABLE/DISABLE`, `SET_BUS_SCALING` | 9, 11, 12, 23 | `0` |
| `GET_QSEOS_VERSION` | 10 | `0x1400000` |
| `APP_LOADED_QUERY` | 15 | `query_mode=0`: `app_id=0` (стек пойдёт на `LOAD_APP`); иначе id как в `LOAD_APP` |

✅SRC Неизвестный ioctl — `-ENOTTY` (тишина вместо ошибки скрыла бы опечатку).

## `qc_send` — сердце шима

1. Пределы: `cmd_len ≤ 64 КБ`, `resp_len ≤ 64 КБ`, `0 ≤ nfd ≤ 4`, иначе
   `-EINVAL` (✅SRC — отказ ДО вызова TZ, не после).
2. `tz_cmd = tzmem(64 КБ)`, `tz_resp = tzmem(64 КБ)`; `copy_from_user cmd`.
3. **Подмена длины тела (Goodix).** Слово по смещению **36** (`GF_POLE_DLINA`)
   при `nfd > 0` поднимается до `fix_dlina = 0x4FFA8`. ✅SRC Зачем: на нашем
   порту малая длина даёт `SENSOR_NOT_AVAILABLE`; proba объявляет `0x4FFA8`.
4. **Тела modfd.** Для каждого `ifd[i]`: `dma_buf_get(fd)`; размер берётся из
   `dmabuf->size`; `tz_len = max(size, объявленная длина)` (✅SRC — вся
   объявленная область обязана принадлежать выделению); `tzmem(tz_len)`,
   обнуление, `begin_cpu_access/vmap/memcpy/vunmap/end_cpu_access`.
5. **Куда пишется физический адрес.** Для Goodix при `cmd_buf_offset == 0` —
   **32-битное** слово в начало заголовка (`tz_cmd[0..4]`), старшая половина
   должна быть нулевой (иначе `-ERANGE`); флаги в `+4` не трогаются. Для
   прочих приложений — 64 бита по `cmd_buf_offset`. ✅SRC; договор подтверждён
   двоичным `goodixfp.flat` @`0x12d8`.
6. `tz_resp` обнуляется перед отправкой (застой из пула не должен выглядеть
   ответом).
7. `qcom_scm_qseecom_app_send(app_id, tz_cmd, cmd_len, tz_resp, resp_len)`.
8. **Обратно:** тело из tzmem в ion (та же пара `begin/vmap/memcpy`), ответ —
   в `resp_buf`, содержимое `tz_cmd` — в `cmd_buf` (ответ иногда лежит там).
9. Освобождение всего, `rc` — первая застрявшая ошибка.

## Что из этого обязан делать Rust

* Не полагаться на `SET_MEM_PARAM`: шим его принимает и забывает.
* Для каждой команды с телом: объявленная длина в заголовке **и** размер
  DMA-выделения согласованы, адрес в начале заголовка 32-битный.
* Проверять **оба** статуса ответа: `+0` — результат команды, `+4` — ошибка
  канала (✅BIN `goodixfp.flat` @`0x1880`, `libgf_ca.so` @`0x2e88`).
