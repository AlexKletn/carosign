# Слушатель QSEE: план реализации

⚠️ Это история слушателя. Действующая спецификация —
[`README.md`](README.md) → `stok/04-jadro-i-sistemy.md`.


Корневой разблокиратор: без него недоступны keymaster (боевой токен) и хранение
образцов; `app_send` к keymaster РОНЯЕТ планшет (§16 RAZBOR). Правило: минимум
в патче ядра, максимум — модуль.

## Точка интеграции (найдена)

`drivers/firmware/qcom/qcom_scm.c`, функция `qcom_scm_qseecom_call` — строки
2158–2161, где сейчас:

```c
WARN_ON(res->result == QSEECOM_RESULT_INCOMPLETE);
WARN_ON(res->result == QSEECOM_RESULT_BLOCKED_ON_LISTENER);
```

Заменить на ЦИКЛ:

```c
while (res->result == QSEECOM_RESULT_BLOCKED_ON_LISTENER ||
       res->result == QSEECOM_RESULT_INCOMPLETE) {
    u32 lid = res->data;            /* при resp_type QSEOS_LISTENER_ID 0xEE02 */
    if (qcom_scm_qseecom_listener_cb)
        qcom_scm_qseecom_listener_cb(lid);   /* модуль обслужил буфер */
    status = qcom_scm_qseecom_listener_continue(lid, res);  /* continue SMC */
    if (status) break;
}
```

## Что в патче ядра (минимум, EXPORT_SYMBOL_GPL)

1. `qcom_scm_qseecom_register_listener(u32 id, phys_addr_t buf, size_t sz)` —
   регистрация буфера слушателя. ABI: owner `QSEE_OS`(50), svc **2**,
   args `{id, buf_phys(RW), size}` (`QCOM_SCM_ARGS(3, VAL, RW, VAL)`).
   ✅ ТОЧНО: SMC `0x32000206` (owner QSEE_OS 50, **svc 2, cmd 6**), из
   `qseecom_ioctl`. Deregister — `0x32000202` (svc 2, cmd 2).
2. `qcom_scm_qseecom_listener_continue(...)` — LISTENER_DATA_RSP.
   ✅ ТОЧНО: SMC `0x320000fc` (owner QSEE_OS 50, **svc 0, cmd 0xfc**) —
   generic-путь, буфер несёт под-команду + listener-response (id, status,
   resp_len). Из `__qseecom_process_incomplete_cmd` (`0x7e1ef0`) с cache-flush
   буфера ДО и ПОСЛЕ (`0x806460`/`0x806550`).
3. Указатель-хук `qcom_scm_qseecom_listener_cb` + регистратор (EXPORT).

## Что в модуле (вся логика)

* выделить общий буфер ~20 КБ (`0x4e20`, §14), зарегистрировать слушателя;
* callback: разобрать запрос ФС из буфера (код операции, путь, смещение,
  длина, данные — раскладка добирается итеративно в userspace/модуле),
  выполнить чтение/запись в `/data/vendor_de/0/fpdata/` (через vfs или проксируя
  в userspace-демон), положить ответ и длину в буфер;
* снять слушателя при выгрузке.

## ⚠️ Перед первым запуском на железе

Три smcid добраны и чисто раскладываются (owner 50 = `0x32`):
register `0x32000206`, deregister `0x32000202`, continue `0x320000fc`. Осталось
добрать при кодировании: точную структуру буфера continue (под-команда,
listener_id, status, resp_len) — она строится функциями стока `0x806928`
(prepare) / `0x806460`,`0x806550` (cache). Это можно итеративно, smcid твёрдые.

## Что разблокирует

* keymaster64 (`app_send` перестанет ронять) → боевой токен gatekeeper;
* `1012 SAVE` / загрузку образцов при `INIT` → постоянное хранение отпечатков.

## Известное ABI (собрано)

| Элемент | Значение | Статус |
|---|---|---|
| точка хука | `qcom_scm_qseecom_call:2160` | ✅ найдено |
| BLOCKED code | `QSEECOM_RESULT_BLOCKED_ON_LISTENER = 2` | ✅ в дереве |
| listener_id | `res->data` при `resp_type = 0xEE02` | ✅ в дереве |
| register svc | 2 (QSEE listener) | ✅ |
| register args | `{id, phys(RW), size}` | ✅ |
| **register** | `0x32000206` = owner QSEE_OS(50)/svc 2/cmd 6 | ✅дв (сканом) |
| **deregister** | `0x32000202` = svc 2/cmd 2 | ✅дв |
| **continue** | `0x320000fc` = owner 50/svc 0/cmd 0xfc | ✅дв |
| буфер | ~20 КБ (`0x4e20`) | ✅дв cmnlib64 |
| cache flush | до и после continue | ✅дв сток |
| функции стока | register/process_incomplete `0x7e1ef0`, data `0x806928/460/550` | ✅дв |

## Реализация — статус (3 сентября 2026)

### Патч ядра — ГОТОВ, собран ✅

`drivers/firmware/qcom/qcom_scm.c`:
* константы `QSEECOM_TZ_SVC_LISTENER = 2`, cmd register=6/dereg=2/continue=0xfc;
* `qcom_scm_qseecom_register_listener()` — SMC `0x32000206`, args {id, phys(RW),
  size}, `EXPORT_SYMBOL_GPL`;
* `qcom_scm_qseecom_unregister_listener()` — SMC `0x32000202`;
* `qcom_scm_qseecom_set_listener_handler()` — регистрация callback модуля;
* хук в `qcom_scm_qseecom_call`: `WARN_ON` заменён ЦИКЛОМ — на
  `BLOCKED_ON_LISTENER`/`INCOMPLETE` зовёт handler и шлёт continue (`0x320000fc`).

Ядро (Image) и Module.symvers собраны, три символа экспортированы ✅.

### Модуль — КАРКАС, собран ✅

`linux-rom/fingerprint/listener/spinel_qsee_listener.c`:
* когерентный буфер 32 КБ, `register_listener(id)`, `set_listener_handler`;
* handler — РАЗВЕДКА: логирует пришедший `listener_id` и голову запроса ФС;
* `listener_id` — module_param (кандидат, подбор по логу).

### ⚠️ Не запускалось на железе

Два неподтверждённых места, проверять ОСТОРОЖНО (риск ребута):
1. **args continue** — сейчас минимальные `{listener_id}`; стоковый путь несёт
   статус в буфере. Если continue не так — TA зависнет.
2. **listener_id** файлового сервиса — кандидат `0x100`; реальный увидим по
   первой блокировке (лог handler'а).

### Порядок безопасной отладки

1. Загрузить модуль БЕЗ обращений к TA — проверить, что register_listener
   проходит (не роняет). Если `register` сам вешает — дело в args/smcid.
2. Малый запрос к goodixfp, вызывающий storage (или keymaster GET_VERSION) —
   поймать `BLOCKED`, увидеть реальный `listener_id` и голову запроса.
3. По логу — поправить id и разобрать раскладку ФС-запроса, реализовать
   файловые операции в handler.

## ⚠️ Правка: register/continue — через generic 0x320000fc с буфером (3 сент)

Шаг 1 на железе: патч механически РАБОТАЕТ — SMC уходит, TZ отвечает, планшет
НЕ падает ✅зам. Но прямой `register` (svc2/cmd6) вернул `-22` (TZ отверг).

Причина найдена ✅дв: стоковый register идёт НЕ прямым desc, а через generic
команду **`0x320000fc`** (owner 50, svc 0, cmd 0xfc) с БУФЕРОМ-запросом
(`__qseecom_scm_call2` вокруг `0x7f0780`). Оба — и register, и continue — этот
же путь, различаются `qsee_cmd_id` в первом слове буфера.

### Верная структура (как app_send, буфер-based)

desc: owner `QSEE_OS(50)`, svc **0**, cmd **0xfc**, args
`{tz_cmd=1, ireq_phys(RW), ireq_len, resp_phys(RW), resp_len}`.

Буфер ireq (классический `qseecom_register_listener_64bit_ireq`):
```
u32 qsee_cmd_id;   // REGISTER_LISTENER (кандидат 4, w8=#4 @0x7f0820) — подтвердить
u32 listener_id;
u64 sb_phys;       // буфер слушателя
u32 sb_len;
```
continue (`listener_data_rsp`): тот же путь, `qsee_cmd_id = LISTENER_DATA_RSP`,
буфер несёт статус/listener_id.

### Переделать в патче

`qcom_scm_qseecom_register_listener` и `..._continue`: не прямой desc, а
generic `0x320000fc` + ireq-буфер из TZmem. `qsee_cmd_id` register/data_rsp —
добрать из стока (`0x7f0780` register, `0x7e1ef0` continue).

Прямые `0x32000206`/`0x32000202` — вероятно старый путь, эта TZ хочет 0xfc.

### Статус

* патч + модуль собраны, ядро развёрнуто ✅;
* register безопасно тестирован — НЕ роняет, TZ `-22` ✅зам;
* правка: путь 0xfc + ireq-буфер; `qsee_cmd_id` подтвердить.

## Итерация register: буфер-ireq (3 сент)

### Подтверждено твёрдо ✅дв

* ireq register = `{u32 qsee_cmd_id=4, u32 listener_id, u64 sb_ptr, u32 sb_len}`,
  32 байта (сток `0x7f0820`: `mov w8,#4`; `w23=0x20`).
* qsee_cmd_id: **REGISTER=4, DEREGISTER=5, LISTENER_DATA_RSP=6** (классический
  QSEOS + `w8=#4` подтверждён).
* register строит ЭТУ структуру и шлёт буфером, НЕ прямыми {id,phys,size}.

### Эмпирика (register безопасен, не роняет)

| Попытка | Ответ |
|---|---|
| прямой svc2/cmd6, args {id,phys,size} | status **-22** (TZ отверг аргумент) |
| буфер-ireq через cmd 0xfc, arginfo{VAL,RW,VAL} | status **-5** (сдвиг!) |

`-22→-5` — ireq-буфер принят лучше; осталась неверная кодировка SMC.

### ⚠️ Три кандидата кодировки (сток)

* `0x32000206` (owner50/svc2/cmd6) — привязан к `qseecom_ioctl` (register);
* `0x320000fc` (owner50/svc0/cmd0xfc) — generic, `process_incomplete`;
* legacy `0x2001001` (base 0x603 + movk 0x200<<16 + 0x9fe) — reentrancy-путь.

### Следующий шаг (без гадания)

Прочитать ТОЧНЫЙ desc register в стоке: у smcid `0x32000206` (`0x7f0884`) —
какие args и arginfo идут в `__qseecom_scm_call2_locked`. Вероятно
**svc2/cmd6 + ireq-буфер как RW-аргумент** (не прямые числа). Затем — один
безопасный тест register.

Ключевое: register не роняет планшет, поэтому кодировку добираем эмпирически
безопасно; continue (в блокирующем цикле) НЕ трогать, пока register не даст 0.

## ✅ REGISTER РАБОТАЕТ (3 сентября 2026)

```
buf phys 0xe6a00000, bridge 0xf46fe
qseecom: register_listener 1: status=0 result=0x0   ← SUCCESS
listener 1 registered
```

### Причина -22 — SHM-мост, НЕ кодировка

Кодировка register была верна С САМОГО НАЧАЛА: SMC `0x32000206` (owner
QSEE_OS/svc 2/cmd 6, собран как `0x32000101 + 0x105` `0x7f3a10` ✅дв), args
`{listener_id, sb_ptr(RW), sb_len}`, arginfo `0x83` (`0x7f34ac` ✅дв). Не
хватало одного: **буфер слушателя должен быть из TZmem с мостом SHM** — как
образ приложения. Без моста TZ отвечает `-22` (адрес не признан).

Итерации `-22 → -5 → 0`: -22 (буфер без моста), -5 (буфер-ireq не туда), 0
(прямой register + буфер с мостом).

### Что теперь есть

* патч `qcom_scm` (register/deregister/continue-обёртки + hook) — работает;
* модуль: TZmem-буфер 32 КБ с мостом SHM, register проходит, слушатель id=1 в
  TZ, callback установлен;
* планшет НЕ падает.

### ⚠️ Continue — НЕ проверен (опасная зона)

`continue` в блокирующем цикле; неверный — ребут. Проверять ТОЛЬКО когда
спровоцируем реальный `BLOCKED` и увидим формат. Порядок:
1. вызвать блокирующую операцию (keymaster GET_VERSION или goodixfp storage);
2. handler залогирует реальный `listener_id` и голову запроса ФС;
3. если `listener_id != 1` — перерегистрировать под него;
4. по логу — выверить continue, затем реализовать файловые операции.

⚠️ keymaster блокируется на СВОЁМ listener (RPMB), id ≠ 1 вероятен. Наш handler
один на все id — залогирует, но continue должен быть верным. Готовить осторожно.

## ⚠️ Урок: continue нельзя проверить без реального обслуживания (3 сент)

Тест continue через keymaster GET_VERSION → ЗАВИСАНИЕ + аварийная перезагрузка.

Причина фундаментальная: keymaster блокируется на СВОЁМ слушателе (RPMB —
защищённый блок eMMC) и ждёт настоящие ДАННЫЕ (прочитанный RPMB), а не просто
«continue, статус 0». Мой continue без данных → TA нечем продолжить → вис.

### Следствия

1. **continue неотделим от обслуживания.** Нельзя послать «готово» с нулями —
   TA ждёт результат операции в буфере. Сначала реально обслужить (файл/RPMB),
   потом continue.
2. **keymaster/RPMB — ПЛОХОЙ полигон.** RPMB требует доступа к защищённому eMMC;
   обслужить его сложно. Не тестировать continue через keymaster.
3. **Нужен точный listener_id** нашего сервиса (goodixfp FS), а не id=1 наугад.

### Пересмотр: слушатель НЕ на критическом пути демонстрации

Малый путь отпечатка (`1060` запись → `1061` опознание в RAM) слушателя НЕ
требует и работает. Слушатель нужен для: постоянного хранения образцов (SAVE) и
боевого пути (keymaster). Оба можно отложить.

### Что осталось для слушателя (когда возьмёмся)

* точный listener_id файлового сервиса goodixfp (по стоку/эксперименту с самим
  goodixfp storage, не keymaster);
* реализация файлового обслуживания в handler (разбор ФС-запроса → vfs);
* continue с РЕАЛЬНЫМ результатом в буфере, не нулями.

⚠️ НЕ тестировать continue, пока handler реально не обслуживает запрос —
иначе зависание.

### Твёрдо достигнуто

* **register РАБОТАЕТ** (status=0) — половина слушателя;
* причина -22 (shm-мост) найдена;
* continue smcid `0x30000001` и args-раскладка разобраны, НО обслуживание —
  предпосылка.

## ✅ Найден референс: libdrmfs.so (3 сент)

Файловый слушатель стока — **`libdrmfs.so`** (из `vendor_b.img`, извлечён
`debugfs`; копия в `current_work/fingerprint-ref/`). Это ПОЛНАЯ референс-
реализация handler'а — можно портировать.

* три сервиса: `fs_start`, `gpfs_start`, `gpfs_static_start`;
* `fs_start(id)`: `QSEECom_register_listener(buf, id, 0x5000, 0)` — буфер
  **0x5000 = 20480** (совпадает с cmnlib `0x4e20`);
* пути: `/data`, `/persist/data/`, `/mnt/vendor/persist/data/`;
* операции: `open/openat/read/write/opendir/readdir/close`, `gpfile_rw_helper`
  (+ backup-файлы);
* **listener_id — параметр** `fs_start(id)`; кандидат **10 (0xa)** — cmnlib64
  write кладёт `mov w0,#0xa` перед отправкой ✅дв (подтвердить).

### План handler'а (порт из libdrmfs)

1. listener_id = 10 (проверить; если goodixfp даст другой — по логу).
2. handler разбирает буфер: `[0]` = код операции, дальше путь/смещение/длина;
   формат — из libdrmfs (разобрать структуру запроса).
3. выполнить операцию через vfs в наш `/data/vendor_de/0/fpdata/`
   (kernel_read/kernel_write или проксировать в userspace-демон);
4. положить результат в буфер, continue с реальным статусом.

### Теперь безопасно тестировать через goodixfp storage (не keymaster!)

goodixfp FS listener проще RPMB: на чтение несуществующего файла handler
вернёт «нет файла», TA продолжит — не зависнет (в отличие от RPMB, что ждёт
реальные данные). Это правильный полигон.

## Архитектурное решение: проксировать в userspace (3 сент)

Разбор `libdrmfs.so`: op = `buf[0]` (байт), jump-таблица по op (0..9),
байтовый разбор пути, backup-файлы, `/data`+`/persist`. Формат запроса/ответа —
сложная упакованная структура; порт этой C++-логики В ЯДРО был бы громоздким и
рискованным.

### Правильный путь (усиливает правило «минимум в ядре»)

**Тонкий ядро-модуль проксирует, userspace обслуживает:**

1. Ядро-модуль (у нас есть): регистрирует слушателя, получает `BLOCKED`,
   выставляет буфер запроса в userspace через char-device (`/dev/spinel-lstnr`:
   read = запрос, write = ответ+статус), будит демон, ждёт ответ, шлёт continue.
2. Userspace-демон: читает запрос, обслуживает файловую операцию (порт
   libdrmfs-логики или прямое использование), пишет ответ. Здесь вся сложность
   формата — в userspace, где ей место.

Плюсы: ядро остаётся тонким (правило соблюдено сильнее), сложный разбор формата
и файловые операции — в userspace (безопасно, отлаживаемо), можно
переиспользовать libdrmfs как образец. Это ровно то, как Android делает
(qseecomd + libdrmfs в userspace).

### Разобрано (референс для демона)

* listener_id кандидат **10**; буфер **0x5000**;
* op = `buf[0]`; операции 0..9 (open/read/write/readdir/close/…);
* пути `/data`, `/persist/data/`, `/mnt/vendor/persist/data/`;
* `libdrmfs.so` в `current_work/fingerprint-ref/` — полный образец.

### Следующие шаги

1. Ядро-модуль: char-device интерфейс (запрос↔ответ) + continue по ответу.
2. Userspace-демон: минимальный обслуживатель (open/read/write в
   `/data/vendor_de/0/fpdata/`), формат из libdrmfs.
3. Тест через goodixfp SAVE (не keymaster): handler↔демон обслужат, continue с
   реальным статусом, TA продолжит.

## Общий статус слушателя

| Часть | Статус |
|---|---|
| патч qcom_scm (register/continue/hook, 3 экспорта) | ✅ собран, register работает на железе |
| register (SMC 0x32000206 + буфер в SHM-мосту) | ✅ status=0 |
| continue (SMC 0x30000001, args разобраны) | ⚠️ требует реального обслуживания |
| listener_id | кандидат 10 (libdrmfs/cmnlib) |
| формат ФС-запроса | референс libdrmfs (op=buf[0], 0..9) |
| обслуживание | → userspace-демон (порт libdrmfs) |

## ✅ Проводник ядро↔userspace РАБОТАЕТ (3 сентября 2026)

```
register_listener 10: status=0                    ← SUCCESS
/dev/spinel-lstnr создан
spinel-lstnrd: ready, waiting for requests        ← демон mmap+poll
```

Вся инфраструктура слушателя собрана и работает на железе ✅зам:

* **модуль** (`spinel_qsee_listener.c`): TZmem-буфер 32 КБ + мост SHM, register
  (id 10) status=0, char-device `/dev/spinel-lstnr` (mmap буфера, read/poll
  запроса, ioctl DONE), handler с таймаутом 5 с;
* **демон** (`spinel-lstnrd.c`, arm64 static): mmap общего буфера, poll,
  разбор op=`buf[0]`, ioctl DONE со статусом;
* планшет стабилен.

### Осталось одно: реальный запрос

Проводник проверен пустым (register + демон ждёт). Полный путь
`BLOCKED → handler → демон → continue` проверяется реальным FS-запросом от
goodixfp storage (SAVE/INIT). Демон пока отвечает ENOENT (безопасно: «нет
файла», TA продолжит, не зависнет — в отличие от RPMB).

⚠️ Осторожный шаг: если listener_id=10 не тот, что у goodixfp FS, TZ
заблокируется на другом id — handler всё равно вызовется (один на все id),
демон ответит, continue пошлётся. Проверять со свежим вниманием.

### Итог инфраструктуры

| Часть | Статус |
|---|---|
| патч qcom_scm (register/continue/hook) | ✅ работает |
| register + SHM-мост | ✅ status=0 |
| char-device проводник (mmap/poll/ioctl) | ✅ работает |
| userspace-демон (разбор/ответ) | ✅ ждёт, разведка |
| continue по реальному запросу | ⚠️ финальный тест |
| реальные файловые операции | → после теста (порт libdrmfs) |

## ⚠️ Тупик отладки: жёсткий ребут без диагностики (3 сент)

Финальный тест (goodixfp INIT провоцирует FS-запрос) → ЗАВИСАНИЕ + жёсткий
ребут, ТРИЖДЫ. pstore пуст (TZ watchdog = полный power-cycle, ramoops не
сохраняется). Диагностировать через ребут НЕВОЗМОЖНО.

### Разделяющий тест показал

goodixfp `stupen=8` зависает **и БЕЗ listener-модуля** на новом ядре. Значит
дело в патче/образе ядра, НЕ в listener-взаимодействии.

Гипотезы (неразделены):
1. hook в `qcom_scm_qseecom_call` ломает штатный goodixfp (хотя при NULL
   handler цикл должен `break`);
2. goodixfp реально блокируется на listener при INIT, а без continue TA
   остаётся висеть в TZ → зависание при следующем вызове;
3. что-то ещё в новом образе.

### Что нужно (свежим заходом, НЕ через ребуты)

1. **Откатить listener-патч полностью**, собрать ЧИСТОЕ ядро, проверить
   goodixfp `stupen=8` — работает ли. Это разделит «патч виноват» от
   «goodixfp сам».
2. Если чистое работает — включать патч по частям (сначала только экспорты без
   hook; потом hook с немедленным `break`) — и смотреть, где ломается.
3. Настроить pers-диагностику: ramoops НЕ переживает TZ-reset; нужен вывод в
   раздел с fsync ДО провокации, либо UART-консоль (физический).

### ⚠️ Правило: не тестировать continue/listener через провокацию блокировки,
пока нет диагностики, переживающей жёсткий ребут. Каждый слепой тест — ребут
без данных.

### Что достигнуто (устойчиво)

* register слушателя РАБОТАЕТ (status=0) — на предыдущем ядре ✅зам;
* проводник (char-device/mmap/демон) РАБОТАЕТ вхолостую ✅зам;
* ABI слушателя разобран (register 0x32000206, continue 0x30000001, ireq);
* референс libdrmfs найден.

Заблокировано на: диагностике зависания (нужен откат + UART/persist-лог).

## Разбор ложной тревоги (3 сент, итог диагностики)

Серия «зависаний» goodixfp оказалась ПУТАНИЦЕЙ, не поломкой:

1. **Реально зависал ТОЛЬКО listener continue** (провокация FS-запроса) — один
   жёсткий ребут.
2. **«up 0 minute» ≠ зависание теста.** Последующие тесты goodixfp запускались
   на свежезагруженном планшете (после того одного ребута), и `up 0 min`
   читался как «завис», хотя тесты проходили. Планшет в покое СТАБИЛЕН
   (uptime растёт) ✅зам.
3. **Откат qcom_scm убрал goodixfp-функции** (`app_shutdown`, `load_services`) —
   app перестал грузиться (ещё одна ложная тревога, не поломка).

### ⚠️ УРОК: мерить uptime ДО/ПОСЛЕ в одном insmod

`up 0 minute` — это свежий uptime после ЛЮБОГО прошлого ребута, НЕ признак
зависания текущего теста. Проверять `B=uptime; insmod; A=uptime; A>B ⇒ жив`.
Иначе один ребут «заражает» интерпретацию всех последующих тестов. Стоило
целого захода ложных следов.

### Восстановленное состояние (рабочее) ✅зам

* goodixfp: `LOADED`, DETECT/INIT/INIT_FINISHED = 0, датчик поднят;
* register слушателя: работает;
* проводник (char-device/mmap/демон): работает вхолостую;
* hook = WARN_ON (continue НЕактивен, не зависает) — безопасно.

### Единственная нерешённая проблема — continue

continue зависает жёстко (TZ watchdog, pstore пуст). Для его отладки нужна
диагностика, переживающая жёсткий ребут: **UART-консоль** (физическая) или
запись в раздел с fsync ДО провокации. Без неё continue не отладить.

Всё остальное (register, проводник, goodixfp) — работает.

## Диагностика диск-логом (3 сент): register конфликтует с app_start

Метод диск-лога (sync после каждой строки) РАБОТАЕТ — переживает жёсткий ребут
✅зам. Показал точку зависания.

### Ключевое наблюдение

* **Без listener-модуля** goodixfp app_start работает (LOADED) ✅зам;
* **С зарегистрированным listener 10** goodixfp app_start ВИСНЕТ — лог
  обрывается на app_start, строки `SPINEL-LSTNR: BLOCKED` НЕТ ✅зам.

Значит: hook не достигается — app_start завис ВНУТРИ SMC (TZ не вернул
BLOCKED), как keymaster. Не «continue неверен», а **сам факт регистрации
listener 10 меняет поведение TZ**: app_start теперь пытается storage-путь и
виснет.

### Гипотеза

listener_id=10, вероятно, НЕВЕРНЫЙ (или обслуживание-заглушка недостаточно).
Регистрация говорит TZ «FS доступен» → goodixfp app_start пытается реальный
storage → наш неполный handler/continue или неверный id → реентрантный вис в
TZ (не BLOCKED, а INCOMPLETE/reentrancy, которую hook не обрабатывает).

### keymaster тоже: вис в SMC, не BLOCKED

keymaster GET_VERSION: диск-лог пуст (ни одной SPINEL строки) — SMC завис в TZ,
не вернул BLOCKED. HLOS-hook недостижим. Оба (keymaster, goodixfp+listener)
виснут ВНУТРИ SMC, а не через возвращаемый BLOCKED.

### Вывод

Проблема НЕ в continue (не достигаем), а в РЕЕНТРАНТНОСТИ: TZ блокировка на
storage идёт как reentrancy (INCOMPLETE / вис в SMC), а не как возвращаемый
BLOCKED_ON_LISTENER. Наш hook обрабатывает только возвращаемый BLOCKED.

### Что нужно (следующий подход)

1. Разобрать по стоку: как TZ ВОЗВРАЩАЕТ управление при storage-запросе —
   BLOCKED (svc/result) или reentrancy (другой механизм). Стоковый
   `__qseecom_process_reentrancy` (`0x7dee98`) — вот ключ.
2. Возможно нужен reentrancy-путь, а не listener-continue.
3. listener_id: правильный (не 10) — но узнать нельзя пока app_start виснет.

### Ценные методы (работают)

* **диск-лог с sync** — переживает жёсткий ребут, показывает до точки виса;
* остановить `spinel-camerad` — камера спамит dmesg;
* uptime до/после — детект ребута.

## hook v2 (реентрантность) — app_start РАБОТАЕТ с listener (3 сент)

Реализован минимальный синхронный путь (спека REENTRANCY.md): цикл по ОБОИМ
исходам — INCOMPLETE (1) и BLOCKED (2), каждый со своим continue. tzmem
когерентен (dma_alloc_coherent) — cache-flush не нужен.

### Прогресс ✅зам (диск-лог)

* goodixfp `stupen=8` БЕЗ listener + hook v2: работает, `SPINEL-RE` не
  триггерится (app_start не реентрантен сам);
* **goodixfp + listener 10 + hook v2: app_start ПРОШЁЛ — `LOADED id=7`!**
  Раньше (hook v1, только BLOCKED) здесь висло. Реентрантная машина помогла.

### Зависание СМЕСТИЛОСЬ дальше

Лог обрывается на:
```
app LOADED id=7, vdd on, sensor powered
error -EINVAL: request_irq(303)   ← proba не смог запросить IRQ (конфликт?)
irq 303: 0                         ← вис ТУТ (на первой команде?)
```

* `SPINEL-RE` строк НЕТ на команде — либо команда виснет в SMC (не вернула
  INCOMPLETE/BLOCKED), либо не дошла.
* `request_irq(303) -EINVAL` — отдельная проблема proba (IRQ занят/конфликт с
  listener-модулем?).

### Следующий шаг

1. Диагностика: какая goodixfp команда виснет (SPINEL-RE логи + диск).
   Возможно DETECT/INIT с listener теперь даёт storage BLOCKED, а continue
   (0x30000001) неверен.
2. request_irq(303) -EINVAL — разобрать (proba IRQ vs listener).
3. Довести continue (BLOCKED data_rsp) — вероятно args/буфер неверны.

### Веха

Реентрантная машина (INCOMPLETE обработка) — app_start с listener больше НЕ
виснет. Это разблокировало главный барьер. Осталось довести continue команд.

## Локализация: INIT (1001) виснет с listener (3 сент)

Диск-лог с диагностикой app_send точно локализовал ✅зам:

```
БЕЗ listener: DETECT 1000 / INIT 1001 / INIT_FINISHED 1005 — ВСЕ rc=0, жив
С listener 10: g2/100 ok, DETECT 1000 ok, INIT 1001 -> app_send [ВИС], нет <-
```

* **INIT (1001) виснет в SMC** с зарегистрированным listener; DETECT (1000) и
  всё до него — проходят;
* `SPINEL-RE` НЕ залогирован → INIT завис в SMC, не вернув INCOMPLETE/BLOCKED
  (как keymaster — вис в EL3), демон запрос НЕ получил;
* app_start реентрантность (INCOMPLETE) hook v2 обработал — app_start LOADED.

### Причина

INIT грузит образцы из storage. БЕЗ listener FS недоступен → INIT пропускает
(rc=0). С listener 10 FS «доступен» → INIT лезет в storage → listener request
с id, который либо не 10, либо требует реального обслуживания → вис.

### Развилка

* **Малый путь РАБОТАЕТ**: goodixfp DETECT/INIT/INIT_FINISHED + команды в RAM
  без listener ✅зам. Для enroll/verify (1060/1061) в RAM слушатель НЕ нужен.
* **Слушатель** упирается в: точный listener_id storage-запроса INIT (не 10) +
  полное обслуживание. id не узнать (INIT виснет в SMC, не возвращает).

### Что достигнуто (реентрантная машина)

* hook v2 (INCOMPLETE+BLOCKED цикл) — app_start с listener РАБОТАЕТ;
* goodixfp полностью работает без listener (малый путь);
* локализован барьер слушателя: INIT storage, listener_id.

### Идея для listener_id (следующий заход)

INIT виснет в SMC (не возвращает id). Способ узнать id БЕЗ виса:
* зарегистрировать НЕСКОЛЬКО кандидат-id, при каком INIT НЕ виснет (демон
  получит запрос) — тот id верный;
* ИЛИ разобрать точный FS listener_id из стока (libdrmfs fs_start caller /
  goodixfp storage init).

## Заход 2 сент: две поломки сняты, корень — модель листенера

Исправлено (read-only обосновано стоком):
1. **continue слушателя** был 0x30000001 (owner TZ_APPS, cmd APP_SEND — чужой
   SMC). Верный: **0x32000203 = TZ_OS_LISTENER_RESPONSE_HANDLER_ID** (owner
   QSEE_OS, svc LISTENER 2, cmd 3), PARAM_ID_2(VAL,VAL) {listener_id, status}.
   Источник: downstream `qseecomi.h` + `__qseecom_reentrancy_process_incomplete_cmd`.
2. **Двойной SHM-мост**: STATIC-пул TZmem САМ мостит область
   (`qcom_tzmem_init_area`→`shm_bridge_create`). Листенер делал ВТОРОЙ мост на
   ту же физпамять → перекрытие → TZ висел в EL3 на ЛЮБОМ app_send. Убран;
   после этого DETECT (1000) снова проходит с активным листенером ✅зам.

Осталось: **INIT (1001) виснет в app_send БЕЗ возврата** (ни BLOCKED, ни
INCOMPLETE; demon не разбужен, SPINEL-RE молчит) — как в самой первой
диагностике. Continue не достигается. DETECT проходит, INIT — нет (INIT читает
калибровку gf_mp_base.so из хранилища).

### Гипотеза: модель обслуживания листенера

Вис-без-возврата в самом SMC ⇒ TA НЕ возвращает управление в EL1 для
обслуживания. Возможные модели QSEE:
* **Реентрантная**: SMC возвращает BLOCKED/INCOMPLETE(data=listener_id), HLOS
  обслуживает, continue. (Так у downstream при qsee_reentrancy_support.)
* **Синхронная (старая)**: TA пишет запрос в SB и КРУТИТСЯ в TZ, ожидая ответа;
  HLOS должен ПОЛЛИТЬ SB отдельным потоком (kthread), а не ждать возврата SMC.

Если наш TZ синхронный — app_send не вернётся, пока листенер не обслужен
поллингом. Тогда наша реентрантная архитектура (ждём BLOCKED-возврат) НЕВЕРНА в
корне: нужен поток-поллер SB.

### Следующий шаг (read-only, без перезагрузок)

1. Прочитать в стоке модель: reentrancy_support версия QSEE; как HLOS
   обслуживает файловый листенер (BLOCKED-возврат vs busy-wait+polling).
2. От этого — либо доработать реентрантный путь, либо сделать поллер SB.

## Итог захода 2 сент: корень сузился до cmnlib64/qsee_sfs

### Надёжно установлено
* **Поллер SB (userspace, весь 32KB, fsync)**: при висе INIT TZ **не пишет в
  наш листенер-буфер ВООБЩЕ** — вис НЕ через обслуживание нашего SB. demon не
  разбужен, ни BLOCKED, ни INCOMPLETE до нас не доходят.
* Значит INIT падает в EL3 РАНЬШЕ возврата к листенеру — внутри самого
  storage-вызова прошивки.

### Зацепка (RAZBOR §14): файловая служба в cmnlib64
* `qsee_sfs_*`/`qsee_fts_*` (Secure File System) живут в **cmnlib64** ✅дв.
  goodixfp зовёт их через `gf_tee_*_object` для `/data/vendor_de/0/fpdata/`.
* Условие виса — ДВА: (1) листенер зарегистрирован (активирует storage-путь;
  без листенера INIT пропускает storage → rc=0), (2) storage-вызов в cmnlib64
  падает в EL3.
* **Сток грузит cmnlib64 ЗАГРУЗЧИКОМ UEFI** (свойство дерева
  `qcom,commonlib64-loaded-by-uefi`), в штатное место. Мы грузим через
  LOAD_SERVICES (cmd 7) в СВОЙ буфер. Вероятно qsee_sfs требует cmnlib64 в
  правильном состоянии/месте — иначе вызов ФС виснет.
* `app_get_id("cmnlib64")=-2` — ЛОЖНЫЙ сигнал: у services-образа нет app_id.
  goodixfp app_start прошёл (не -22) ⇒ cmnlib64 скорее загружена, но её ФС-путь
  не готов.

### Следующий заход (READ-ONLY сначала, беречь перезагрузки!)
1. Разобрать в стоке `qsee_sfs_open`/`gf_tee_storage`: что именно вызывается в
   INIT, и почему падает без штатной cmnlib64.
2. Проверить `qcom,commonlib64-loaded-by-uefi` в нашем дереве: грузит ли UEFI
   cmnlib64, и куда. Возможно надо грузить cmnlib64 как сток (загрузчиком), а не
   LOAD_SERVICES.
3. Только потом — железный тест (1 перезагрузка на гипотезу, не больше).

### Исправлено за заход (в дереве, готово)
* continue слушателя 0x30000001 → **0x32000203** (LISTENER_RESPONSE_HANDLER).
* Двойной SHM-мост в модуле листенера убран (STATIC-пул мостит сам).

## Разбор qsee_sfs_open и загрузки cmnlib64 через UEFI (2 сент, read-only)

### qsee_sfs_open — файловая служба в cmnlib64
* Набор ✅дв (строки в cmnlib64.b0x): open/close/read/write/seek/getSize/rm/
  mkdir/rmdir/get_file_list/rename + qsee_fts_* (encrypt/decrypt/integrity).
* Реализация open @~0x29ab4 (образ собран по p_offset: cmnlib64.bin, code
  0..0x6d400). Обёрнута трассировкой (adrp x1=fmt@0x60000, x2=имя@0x67000).
* Механизм: sfs НЕ трогает диск — кладёт запрос ФС в буфер листенера и через
  gp-callback возвращает в HLOS как BLOCKED_ON_LISTENER(id); HLOS-демон делает
  файловую операцию, шлёт continue. `gpGetListenerSize` (@0x18840) — cmnlib
  СПРАШИВАЕТ у QSEE-OS размер зарегистрированного листенер-буфера через
  gp-callback таблицу, которую QSEE-OS передаёт cmnlib при её init.

### Загрузка cmnlib64 через UEFI
* `qcom,commonlib64-loaded-by-uefi` (в стоковом дереве, source_artifacts) →
  `qseecom.commonlib64_loaded = true` (qseecom.c:9418) → драйвер ПРОПУСКАЕТ
  `qseecom_load_commonlib_image("cmnlib64")` (условие `!commonlib64_loaded` на
  2790). Причина: **appsbl/UEFI грузит cmnlib64 САМ** на этапе загрузчика из
  non_hlos.bin, ДО Linux, и QSEE-OS инициализирует её gp-callback связку.
* Без свойства драйвер грузил бы сам: `QSEOS_LOAD_SERV_IMAGE_COMMAND` по
  phy_addr через SCM_SVC_TZSCHEDULER (legacy). proba грузит через новый ABI
  (LOAD_SERVICES, 0x32000107).

### Вывод для виса INIT
На стоке cmnlib64 инициализируется ЗАГРУЗЧИКОМ штатно ДО app; QSEE-OS связывает
её с gp-callback (в т.ч. доступ к листенерам). У нас cmnlib64 грузится ПОЗЖЕ из
Linux (LOAD_SERVICES). Гипотеза: **её gp-callback init не отрабатывает как при
appsbl** → при qsee_sfs → gpGetListenerSize cmnlib получает неверную таблицу →
выход за буфер/вис. cmnlib64.* ЕСТЬ в /lib/firmware планшета (request_firmware
их находит).

### Следующие гипотезы (по одной, беречь перезагрузки)
1. Грузить cmnlib64 ТЕМ ЖЕ путём, что сток (legacy TZSCHEDULER +
   QSEOS_LOAD_SERV_IMAGE_COMMAND), а не новым LOAD_SERVICES — вдруг новый ABI не
   даёт полной gp-init.
2. Проверить порядок: cmnlib64 ДО регистрации листенера И до app_start; сейчас
   листенер регистрируется отдельным модулем — момент относительно cmnlib64 не
   выверен.
3. Крайнее: грузить cmnlib64 в загрузчике spinel (UEFI-путь), как appsbl.

## Гипотеза №1 (2 сент): app_region — новый факт, вис не снят

* Перенёс `set_app_region` ПЕРЕД загрузкой cmnlib (как сток — в probe до всего).
* **Результат: `app_region raw: status=-22`** (EINVAL) — регион НЕ регистрируется.
* Причина ✅дв: наша `qcom_scm_qseecom_set_app_region` шлёт НОВЫМ ABI
  (`TZ_OS_APP_REGION_NOTIFICATION_ID`=0x32000105, довод `BUF_RW`), а whitelist-TZ
  требует адрес в SHM-bridge; стоковый secapp-region 0x82400000 в наш bridge не
  входит → -22. **Сток шлёт app_region LEGACY-путём** (`SCM_SVC_TZSCHEDULER`,
  без whitelist) — оттого у него проходит.
* Смена ABI загрузки cmnlib (сама гипотеза №1) отпадает: наш LOAD_SERVICES
  (0x32000107) ТОЧНО совпадает со стоковым `TZ_OS_LOAD_SERVICES_IMAGE_ID`, и
  cmnlib реально грузится (app_start=LOADED, иначе -22). Транспорт не при чём.
* Вис INIT (1001) не изменился.

### Уточнённая развилка
Настоящее расхождение со стоком — **app_region не регистрируется** (у нас -22).
cmnlib грузится в регион, о котором TZ не уведомлён штатно → возможная причина
виса sfs. Варианты довести:
  (1a) app_region LEGACY-путём (как сток) — обойти whitelist. Правка ЯДРА
       (новая legacy-обёртка) → пересборка → 1 перезагрузка.
  (1b) SHM-bridge на 0x82400000 ПЕРЕД app_region — правка proba (модуль, без
       перезагрузки ядра). Дёшево, пробуем первым.
Либо перейти к гипотезе №2 (порядок листенер↔cmnlib) или №3 (грузить cmnlib в
загрузчике). Но app_region -22 стоит закрыть — он необходим для стоковой
полноты cmnlib.

## Итог 1b: app_region НЕ корень; исключено многое

* Мост на secapp-region 0x82400000 → **74 «SHM bridge IS in use»**: диапазон уже
  замощён системно. app_region всё равно **-22** ⇒ -22 = «регион уже установлен»
  (загрузчиком/системой), а не проблема адреса. app_region как корень — снят.
* register листенера 0x32000206 (SMCINVOKE) — **корректен**: downstream шлёт
  ровно его первым, откат на 0x32000201 только при -EOPNOTSUPP. Листенер
  зарегистрирован правильно.

### Что теперь ИСКЛЮЧЕНО (всё ✅зам/✅дв)
continue (0x32000203 верный) · двойной мост (убран) · register (корректен) ·
cmnlib64 (загружена, app_start=LOADED) · app_region (системно уже есть) ·
TZ НЕ пишет в наш SB при висе (поллер всего буфера).

### Загадка в чистом виде
Регистрация листенера 10 → INIT(1001) виснет в app_send БЕЗ возврата и БЕЗ
записи в наш SB. Всё вокруг листенера корректно. Значит INIT входит в
storage-путь, который падает в EL3 ВНУТРИ прошивки/cmnlib, не дойдя до нашего
SB, и лишь ФАКТ наличия листенера 10 включает этот путь.

### Оставшиеся дешёвые кандидаты
1. **Конфликт listener_id=10** — тест с другим id (параметр insmod, .ko готов):
   если INIT перестанет виснуть → 10 занят системно. 1 сброс.
2. **Read-only: что делает GF_CMD_INIT (1001) в стоке** — трогает ли storage/
   листенер, ИЛИ виснет по иной причине. Уточнит, тот ли это путь.
3. Инфраструктура листенера (kthread'ы unregister/unload, как у стока) —
   возможно обслуживание требует их.

## Разбор GF_CMD_INIT (1001) в стоке (2 сент, read-only)

Образ goodixfp собран (7.66 МБ, scratchpad/goodixfp.bin). Обработчик INIT ✅дв:
* **INIT → NATIVE_SERVICE_INIT_HAL**: поднимает HAL, включая файловое хранилище.
* Рядом в обработчике (@0x33b1c1..) — вся файловая цепь ошибок ✅дв:
  `GF_ERROR_MKDIR_FAILED`, `FILE_OPEN_FAILED`, `FILE_NOT_EXIST`,
  `FILE_READ/WRITE/DELETE_FAILED` + `mkdir` (@0x33ba21).
* Калибровка/OTP: `CALIBRATION_NOT_READY`, `INVALID_OTP_VERSION`,
  `OTP_CHECK_FAIL`, `NO_NEED_UPDATE_BASE` — INIT ЧИТАЕТ калибровочную базу и OTP
  ИЗ ФАЙЛА.
* Файловые примитивы: `gf_tee_storage` (@0x3397c5), `gf_tee_open` (@0x3398a0),
  `qsee_sfs` (@0x741d35).

### Вывод
INIT(1001) ЛЕГИТИМНО и ОБЯЗАТЕЛЬНО входит в storage: сперва **mkdir каталога**
`/data/vendor_de/0/fpdata/`, затем **чтение калибровки/OTP из файла**. Всё через
`gf_tee_storage → qsee_sfs` (cmnlib) → листенер. Значит:
* листенер для INIT НЕОБХОДИМ (это тот самый путь, вис не ложный);
* КАДР (изображение) НЕ нужен — подтверждает прежнее;
* вис = `qsee_sfs` (mkdir/open) падает В cmnlib ДО записи в наш SB (поллер пуст)
  ⇒ связка cmnlib↔листенер (gp-callback, gpGetListenerSize) не инициализирована
  при нашей загрузке cmnlib из Linux. app_region тут ни при чём (он системно
  есть). Корень — **gp-инициализация cmnlib-стороны листенера**.

### Куда бить дальше
Как cmnlib получает gp-callback таблицу листенер-сервисов: при LOAD_SERV TZ
передаёт её, ЛИБО она ставится при регистрации листенера в правильном
порядке/контексте (сток: kthread'ы + probe-порядок). Проверить порядок
«register listener ДО первого storage-app_send» и полноту gp-init.

## Порядок листенер↔cmnlib: гипотеза отпала (2 сент, read-only)

* В тестах листенер (модуль) регистрируется ПЕРЕД proba, cmnlib грузится ВНУТРИ
  proba ⇒ **листенер УЖЕ до cmnlib**, вис всё равно. Простой порядок не помог.
* Сток — ОБРАТНО: загрузчик грузит cmnlib64 при boot, qseecomd регистрирует
  листенеры ПОЗЖЕ (Linux). Т.е. на стоке cmnlib ДО листенеров.
* Разбор `gpGetListenerSize` в cmnlib (@0x18800) ✅дв: это **рантайм-запрос**
  размера листенера через gp-callback; при неудаче — `mov w19,#0xffff0006`
  (-65530), graceful ВОЗВРАТ ошибки, НЕ вис. Кеша-при-init нет.
* ⇒ Порядок «листенер до/после cmnlib» саму связку НЕ определяет: cmnlib
  спрашивает листенер в момент sfs-вызова. И «листенер не виден» дал бы -65530
  (ошибку INIT), а у нас ВИС (EL3 fault) — грубее.
* Рядом ✅дв: cmnlib для sfs использует `gp_register_and_prepare_shared_buffer_
  for_TA` / `qsee_register_shared_buffer` — отдельный shared buffer cmnlib↔TA
  (не наш листенер-SB).

### Вывод
Вис INIT — НЕ graceful-промах листенера (тот дал бы -65530) и НЕ порядок. Это
EL3 fault внутри storage-пути. Кандидаты сузились до грубого сбоя доступа к
памяти в TZ при storage с активным листенером. Порядок/cmnlib-загрузка/app_region
— исключены.

## Находка: meta-qcom / minkipc — исходники файлового listener (2 сент)

github.com/qualcomm/minkipc → listeners/libfsservice — ИСХОДНИКИ файлового
listener (то, что я реверсил из libdrmfs). Скопировано в
current_work/fingerprint-ref/minkipc-listener/ (fs_msg.h, gpfs_msg.h, fs.c,
gpfs.c, cmn.h).

### Подтверждено исходниками (снимает подозрения)
* **FILE_SERVICE_ID = 0xa = 10** — наш listener_id=10 ВЕРНЫЙ ✅.
* **FILE_SERVICE_BUF_LEN = 20*1024 = 0x5000** — совпадает с реверсом libdrmfs.
* Формат запроса: cmd_id@0, pathname[256]@4, offset@260, count@264 (TZ_CM_MAX_
  NAME_LEN=256) — подтверждает мой SB-реверс (@260 = 4+256).
* Команды fs: FILE_START=0x201, OPEN/READ/WRITE/CLOSE/LSEEK/MKDIR/FSTAT/RMDIR/
  RENAME/... (cmd_id первым словом). gpfs: id=0x7000, DATA/PERSIST FILE R/W.
* fs.c/gpfs.c — реализация обслуживания (образец для нашего demon вместо ENOENT).

### НЕ применимо к нам
* Транспорт minkipc = QTEE object-invoke через TEE-subsystem (qcomtee driver),
  для НОВЫХ чипов (SM8650+). Наш SM8250 = qseecom. Register/continue/вис —
  qseecom-специфичны, minkipc их НЕ решает.
* goodixfp у нас — qseecom TA (app_start + qseecomd/libdrmfs listener). Наш путь
  (qseecom listener в ядре) — верный для SM8250.

### Что это даёт
1. id=10 и buf=20KB подтверждены (не в них причина виса).
2. Точный протокол fs/gpfs → написать demon ПРАВИЛЬНО (не заглушку), когда
   решим вис. Портировать fs.c-обслуживание.
3. Вис (EL3 при qseecom storage) остаётся qseecom-специфичным — тут minkipc не
   помощник.

## РЕШАЮЩИЙ факт (2 сент): виснет ИМЕННО id=10

Железный тест listener_id ✅зам:
* **listener_id=10**: INIT(1001) виснет → EL3 fault → сброс.
* **listener_id=4660** (0x1234, не системный): DETECT, INIT 1001 **rc=0**,
  INIT_FINISHED — ВСЁ проходит, планшет ЖИВ (uptime 8109→8114).

⇒ Регистрация листенера САМА по себе storage не ломает. Ломает **занятие
id=10**. id=10 (FILE_SERVICE_ID=0xa) — особый системный слот drmfs.

### Интерпретация
* С id≠10: INIT ведёт себя как БЕЗ листенера (storage пропущен gracefully,
  rc=0) — наш листенер не в слоте файловой службы, goodixfp/cmnlib его не зовёт.
* С id=10: наш листенер попадает в слот, к которому идёт cmnlib-sfs. И тут —
  вис, причём TZ НЕ пишет в наш SB (поллер). Значит наш register(10) либо
  КОНФЛИКТУЕТ с уже существующим листенером 10 (системный/cmnlib), либо TZ ждёт
  для слота 10 иную конфигурацию буфера/регистрации, и падает при обращении.

### Следующий прицельный шаг
Проверить, ЗАНЯТ ли id=10 до нас:
* unregister(10) ПЕРЕД register — если вернёт success, 10 был кем-то занят
  (cmnlib при загрузке? системный?). Тогда снять и зарегистрировать чисто, ИЛИ
  вовсе не регистрировать (если cmnlib сама держит HLOS-конец — но у нас нет
  qseecomd, значит HLOS-конец наш).
* Либо: register(10) БЕЗ загрузки cmnlib (stupen<5) — виснет ли? Если нет —
  значит именно cmnlib занимает/настраивает слот 10.

## Уточнение: unregister неинформативен; id=10 — слот файловой службы

* `unregister(id)` возвращает **0 для ЛЮБОГО id** (9999, 55555 — заведомо
  свободные) ⇒ pre-unregister(10)=0 НЕ доказывает занятость. Откачено.
* Твёрдо (железный тест): **id=10 → вис, id≠10 → INIT rc=0, жив**. id=10 =
  FILE_SERVICE_ID=0xa (подтверждён исходниками minkipc) — слот файловой службы,
  к которому идёт cmnlib-sfs.
* Механизм: с id=10 наш листенер в слоте fs → cmnlib-sfs при INIT НАЧИНАЕТ
  storage-транзакцию (видит листенер) → падает в EL3 ДО записи в наш SB (поллер
  пуст). С id≠10 sfs не видит листенер 10 → storage пропущен gracefully → rc=0.

### Кандидаты (почему транзакция к слоту 10 падает)
1. **Размер/адрес SB**: cmnlib ждёт для fs буфер РОВНО FILE_SERVICE_BUF_LEN=
   0x5000 (20480). Наш register(10) даёт 32KB. Проверить register с точно 0x5000.
2. **Механизм обслуживания слота 10**: возможно cmnlib-fs обслуживается НЕ
   через qseecom-BLOCKED, а через QTEE/smcinvoke object (minkipc-модель). Тогда
   qseecom register(10) регистрирует не в той таблице, и cmnlib падает.
3. Наш register через SMCINVOKE (0x32000206) ставит smcinvoke_support-контекст;
   возможно для слота 10 нужен ОБЫЧНЫЙ register (0x32000201).

## ⭐ Гипотеза RPMB (2 сент): файловая система ВЛИЯЕТ через secure-storage backing

Вопрос «влияет ли ФС» — ДА, но не /data, а **RPMB (anti-rollback backing)**.

### Улики
* cmnlib завязан на RPMB ✅дв: `qsee_rpmbw_open_partition/read_sectors/...`,
  `version_Init/Open/Read/Write_Rpmb_Partition`, `cmnlib_gppo_rpmb_enablement`,
  `qsee_sfs_is_anti_rollback_enabled`, `gpd.tee.trustedStorage.antiRollback`.
* Шаблоны отпечатков хранятся с anti-rollback (защита от replay) → RPMB.
* Стоковый supplicant (minkipc listener_mngr) поднимает: Time, TA-Autoload,
  **FS id=0xa**, **GPFS id=0x7000**, **RPMB id=0x2000**. RPMB команды: INIT
  0x101, READ 0x102, WRITE 0x103, PARTITION 0x104. BUF = DATA+40.
* qseecomd: «RPMB_INIT failed, shall not start listener services» — RPMB
  ПЕРВИЧЕН и обязателен для файловых листенеров.
* Планшет: RPMB железно есть (UFS W-LU 0:0:0:49488 → sg0, geometry
  rpmb_rw_size), но HLOS его НЕ обслуживает (нет /dev/rpmb, нет демона, нет
  RPMB-листенера 0x2000).

### Как объясняет вис id=10
fs-транзакция (mkdir/open fpdata) ВНУТРИ дёргает anti-rollback → RPMB листенер
0x2000. С листенером fs(10) sfs НАЧИНАЕТ транзакцию → доходит до RPMB (нет
0x2000) → EL3 fault ДО записи в наш SB. С id≠10 sfs не стартует → RPMB не
трогается → graceful rc=0. Это ровно наблюдаемое.

### Подтверждающий тест
Зарегистрировать ОБА листенера — fs(0xa)+RPMB(0x2000) (заглушки+поллер SB).
Если вис уходит ИЛИ поллер RPMB-буфера ловит запрос (INIT 0x101) → RPMB-путь
подтверждён. Дальше — обслуживать RPMB (librpmbservice образец, UFS SECURITY
PROTOCOL через sg0) ИЛИ отключить anti-rollback.

Файлы: current_work/fingerprint-ref/minkipc-listener/rpmb_service.h + fs/gpfs.

## Тест fs+rpmb листенеры (2 сент): вис ДО HLOS, handler не зван

* register fs(10)=0 И register RPMB(0x2000)=0 — оба листенера встали.
* INIT(1001) ВСЁ РАВНО виснет; `SPINEL-RE-LSTNR` молчит (ни FS, ни RPMB handler
  не вызван). TZ не пишет ни в один SB.
* ⇒ cmnlib-sfs падает в EL3 **ВНУТРИ TZ, ДО выхода в HLOS-листенер**.
  Регистрация HLOS-листенеров (fs/rpmb) НЕ помогает — до них не доходит.

### Переосмысление
Вис — не «нет HLOS-листенера». cmnlib при sfs с активным листенером 10 падает в
своей TZ-логике раньше HLOS-вызова. Кандидаты:
1. **RPMB device init ВНУТРИ TZ** (`version_Init_Rpmb_Partition`,
   `qsee_rpmbw_add_partition`) — синхронная инициализация RPMB при первой sfs
   операции; без реально provisioned/инициализированного RPMB backing падает.
   HLOS-листенер-ЗАГЛУШКА тут не спасает — нужен настоящий RPMB (authenticated
   frames) ИЛИ отключить anti-rollback.
2. Природа/место SB листенера 10 (тип памяти), которую cmnlib читает при setup.
3. Внутренний gp-callback cmnlib падает.

### Направление
* Проверить, можно ли ОТКЛЮЧИТЬ anti-rollback (тогда sfs не трогает RPMB).
* Либо поднять НАСТОЯЩИЙ RPMB (provision + обслуживание, librpmbservice + UFS
  SECURITY PROTOCOL sg0) — крупная работа.

## ⭐⭐ Заход RPMB/сеть (2 сент): cmnlib ПАТЧИБЕЛЬНА, anti-rollback путь найден

### Победы (крупные)
1. **Сеть дала ключ:** anti-rollback SFS отключается флагом
   `cmnlib_gppo_rpmb_enablement` (Enabled/Disabled, default Enabled), файл
   `cmnlib_oem_config.xml` в TZ-образах. Функция-проверка:
   `version_Is_Rpmb_Usage_Enabled` (@0x22a74) → зовётся из
   `qsee_sfs_is_anti_rollback_enabled` (@0x2a25c, BL @0x2a46c).
2. **cmnlib ПАТЧИБЕЛЬНА — подпись НЕ блокирует!** ✅зам: пропатчил
   version_Is_Rpmb_Usage_Enabled + qsee_sfs_is_anti_rollback_enabled →
   `mov w0,#0; ret` (return 0) в cmnlib64.b02, залил в /lib/firmware, proba
   загрузила (load_services=0, goodixfp LOADED). md5 подтверждён на планшете.
   ⇒ можем модифицировать cmnlib как ICP. Огромно для будущего.
3. Подтверждено сетью: RPMB listener id=**0x2000** (8192), sb_len=**0x6400**;
   RPMB через listeners+qseecomd; RPMB provision one-time (планшет вероятно
   provisioned). Community (sm8250/sdm845-mainline) qseecom TA storage НЕ решил.

### Отрицательный результат
Патч обеих anti-rollback функций (return 0) НЕ убрал вис INIT(1001). ⇒ RPMB
зовётся НЕ через флаг anti_rollback: sfs при mkdir/open инициализирует RPMB
НАПРЯМУЮ (`version_Init_Rpmb_Partition`, `qsee_rpmbw_open_partition`),
независимо. ЛИБО вис вообще не от RPMB.

### Следующая итерация
* Патчить сам RPMB init напрямую: `version_Init_Rpmb_Partition` /
  `qsee_rpmbw_open_partition` → return success(0) (skip RPMB). Риск: cmnlib
  дальше использует "успешный" RPMB и падает глубже.
* ЛИБО подтвердить, доходит ли sfs до RPMB (с патчем anti_rollback) или падает
  раньше — трассировкой в пропатченной cmnlib (patch точки на pr/log).
* Стоковый cmnlib — бэкап в /root/cmnlib-stock-bak/ на планшете.

## tzlog взят в работу (2 сент): обёртка есть, TZ log упёрся в 2 барьера

### Сделано
* **Обёртка `qcom_scm_qseecom_register_log_buf(phys,size)`** в qcom_scm.c
  (owner QSEE_OS, svc APP_MGR, cmd 6, ARGS(2,RW,VAL)) — EXPORT, в ядре. SCM
  rc=0 ✅. Прототип в qcom_scm.h.
* **Модуль `linux-rom/fingerprint/tzlog/spinel_tzlog.ko`** — register qsee_log
  буфер + kthread-поллер в dmesg.
* Разобран стоковый tz_log.c: diag region **0x146bf720 размер 0x3000** (DTS
  `tz-log@146bf720`, "qcom,tz-log"); структуры tzdbg_t/tzdbg_log_t; SCM ids.

### Два барьера (TZ log не получен)
1. **Plain qsee_log register: rc=0, но QSEE НЕ пишет** (буфер остаётся 0). ⇒
   наш TZ, вероятно, требует **encrypted log** (`is_encrypted_log_enabled`,
   query feature SMC). Plain путь молчит.
2. **ioremap diag 0x146bf720 РОНЯЕТ планшет** (даже 4 байта magic, выровненный).
   ⇒ region secure на нашем xbl (сток ioremap'ит успешно — там открыт HLOS). Это
   та самая «TZ halt less-privileged Linux» природа (sdm845 issue).

### Следующий заход
* **Encrypted log path**: query encrypted feature SMC → request encrypted log
  buffer (ENCRYPTED_QSEE_LOG_ID=1) → TZ пишет зашифрованно → HLOS читает/
  расшифровывает (downstream: request_encr_log SMC, print_text). Строки
  1090-1215 tz_log.c. Доп. объём, но конкретный путь.
* Либо: сделать diag region HLOS-доступным (xbl/dts memory-region) — сложнее.

## Workflow encrypted-tzlog + эксперимент (2 сент): TZ log недостижим

Workflow (5 разборщиков + синтез) дал точный план: обёртки query_encr_feature
(0x3200010B, svc APP_MGR cmd 0x0B, ARGS(0)) и request_encr_log (0x3200010C, cmd
0x0C, ARGS(3,RW,VAL,VAL), args{phys,size,log_id=1}). Реализовано (2 обёртки в
qcom_scm + модуль). Первый эксперимент ✅зам:

* **query_encr_log_feature = status -5 (EIO), без сброса.** register_log_buf
  (cmd 6) давал rc=0, а cmd 0x0B отвергнут → наш TZ **НЕ поддерживает
  encrypted-log** (фича новее нашего SM8250 TZ 2020).

### Все три пути к TZ log ЗАКРЫТЫ (итог)
1. plain qsee_log register (cmd 6): rc=0, но QSEE НЕ пишет (log_pos=0).
2. ioremap diag 0x146bf720: РОНЯЕТ (secure на нашем xbl).
3. encrypted log (cmd 0x0B): -5 (не поддерживается).
+ даже при успехе encr — шифртекст AES-256-GCM, ключ RSA-обёрнут, HLOS НЕ
  расшифрует без OEM-ключа (off-target).

### Вердикт направления
Читаемый TZ log на нашем защищённом устройстве **недостижим**. Значит точку виса
cmnlib RPMB/storage изнутри TZ не увидеть. Это граница безопасности устройства,
а не нехватка усилий: cmnlib storage виснет в TZ, TZ log закрыт тремя способами,
расшифровка требует OEM-ключа. Community (sm8250/sdm845-mainline) это тоже не
прошёл.

### Что осталось в дереве (полезное)
* qcom_scm обёртки: register_log_buf, query_encr_log_feature, request_encr_log,
  register_listener/continue (0x32000203), load_services, app_region.
* Модули: spinel_qsee_listener (fs+rpmb), spinel_tzlog, spinel_fp_proba.
* cmnlib ПАТЧИБЕЛЬНА (подпись не блокирует) — рычаг для будущего.

## Разведка OnePlus SM8250 kernel (2 сент) — крупные находки

Репо: github.com/OnePlusOSS/android_kernel_oneplus_sm8250 (oneplus/SM8250_R_11.0).

### Подтверждено / ново
1. **RPMB listener = 0x2000, SSD = 0x3000** — ядро спец-обрабатывает ТОЛЬКО их
   (и то лишь clock-gate, а у нас `qcom,no-clock-support` → no-op). file-service
   id (наш 10) определяется в userspace, ядру всё равно. → наш 0x2000 ВЕРЕН.
2. **RPMB данные идут через HLOS-listener 0x2000, in-kernel RPMB servicing НЕТ.**
   Значит обслуживать 0x2000 надо реальными UFS RPMB кадрами (SECURITY PROTOCOL
   0xEC, /dev/bsg/0:0:0:49476).
3. **RPMB provision — отдельный ПРЯМОЙ SCM** (TZ_SVC_RPMB=4:
   TZ_OS_RPMB_PROVISION_KEY_ID, _ERASE_ID, **_CHECK_PROV_STATUS_ID**). Ядро НЕ
   провижинит на boot. ⇒ можем БЕЗОПАСНО спросить статус провижинга (диагностика).
4. **Реентрантность фаза = 2** (DT `qcom,qsee-reentrancy-support=<2>`). Фаза ≥1
   ДРОПАЕТ app_access_lock во время обслуживания listener. Неверная фаза/continue
   → рассинхрон TZ↔HLOS → watchdog. Наш хук держит lock вокруг handler — сверить.
5. **Зарегистрированный, но битый listener блокирует НАВСЕГДА** (wait_event без
   таймаута на listener_in_use). Будущая ловушка для нашего stub.
6. **cmnlib64 РЕЗИДЕНТНОСТЬ — smoking gun.** goodixfp 64-бит → INIT зовёт cmnlib64.
   Сток: `qcom,commonlib64-loaded-by-uefi` → ядро НЕ грузит, доверяет ABL/XBL.
   Если наш boot-chain не сделал cmnlib64 резидентной корректно → INIT прыгает в
   несуществующую/битую cmnlib64 → EL3 fault. Совпадает с симптомом (cmnlib умирает
   ДО вызова любого listener). ⚠️ Мы грузим LOAD_SERVICES (rc=0), но НЕ подтвердили,
   что патч/образ реально исполняется.
7. **Goodix kernel driver — тупой GPIO/netlink shim, БЕЗ secure-SPI.** ⇒ вис
   ТОЧНО в TZ storage, НЕ в SPI к сенсору. Одна теория закрыта.

### Безопасные следующие тесты (из отчёта)
* **QSEOS_RPMB_CHECK_PROV_STATUS_COMMAND** (TZ_OS_RPMB_CHECK_PROV_STATUS_ID) —
  прямой SCM, без listener, без сброса: провижинена ли RPMB. Отделяет «ключа нет»
  от «cmnlib64 не резидентна».
* Дизамбигуация: cmnlib64 загружена + listener'ы НЕ зарегистрированы → по коду
  missing listener даёт чистый -EINVAL, НЕ вис. У нас без listener INIT=rc=0
  (наш shim глотает BLOCKED?). Сверить с фазой 2.

### Пересмотр механизма виса
INIT app_send виснет в SMC ДО возврата BLOCKED (SPINEL-RE молчит) ⇒ cmnlib
падает в TZ ДО выдачи любого listener-callback. Кандидаты (OnePlus): cmnlib64
не резидентна корректно, ЛИБО RPMB не провижинена, ЛИБО реентрантность-фаза 2.

## Разведка OnePlus Nord (2 сент) — ODM ПОДТВЕРЖДЁН

Гипотеза общего ODM ПОДТВЕРЖДЕНА: Nord (avicii, SM7250) и OP8 (sm8250-common,
kona = НАШ SoC) везут **байт-идентичный odm-пакет биометрии**. Доказательство:
avicii несёт ТОТ ЖЕ `CAMERA_ICP.elf`, что и наш планшет. ⇒ наш планшет — эта
ODM-семья. Для TZ/cmnlib брать **sm8250-common** (наш SoC).

### Ключевое
* Сенсор: Goodix **GF9558, G3, подэкранный оптический** (Milan) — совпадает.
* HAL: `fingerprint.20801_goodix9558_G3.default.so`, `libgf_ud_hal_..G3_oplus.so`.
* **Конфиг storage = `gpfspath_oem_config.xml`** (НЕ cmnlib_oem_config), verbatim:
  - `gp_data_path = /data/vendor/tzstorage/`
  - `gp_persist_path = /mnt/vendor/persist/data/`
  - `gp_whitelist = /data/system/users/, /data/misc/qsee/, /qwes, /qwes/licenses`
* FP-каталоги (init.rc): `/data/vendor/fingerprint`, `/mnt/vendor/persist/fingerprint`.
* Демоны boot: **qseecomd** (регистрирует listener'ы RPMB/GP-FS/time ДО TA),
  **rmt_storage** (`shutdown critical`, `ioprio rt 0`) — backing TZ/modem storage.

### Интерпретация виса (сходится с OnePlus SM8250)
На этом ODM cmnlib-storage при INIT достигает HLOS через **GP-FS/SFS listener**
(пишет в /data/vendor/tzstorage + /mnt/vendor/persist/data), а RPMB — backing
ПОД ним (qseecomd+rmt_storage, уже провижинена). Оттого:
* наш RPMB-0x2000 handler не зван — путь SFS, не прямой RPMB round-trip;
* cmnlib «умирает до HLOS» т.к. `version_Init_Rpmb_Partition` бьётся о RPMB,
  которую наш mainline-boot не поднял (нет qseecomd/rmt_storage) → EL3 fault.
  Listener-callback'и QSEE СИНХРОННЫ/блокирующие: если полный набор listener'ов
  + разделяемые буферы + RPMB-ключ не подняты ДО TA — TZ спинит → watchdog.

### Питание сенсора (Nord platform.c, для проекта 20801 = наш?)
GPIO-гейт LDO (НЕ regulator_get): pm8150 gpio7=VDD(3.024В через pm8150a_l9),
gpio6=1.8В, сброс tlmm63, IRQ tlmm112. PMIC pm8150/pm8150a = как у нас (kona).

### СХОДИМОСТЬ обоих разведчиков → приоритетные рычаги
1. **RPMB_CHECK_PROV_STATUS** (прямой SCM, БЕЗОПАСНО) — провижинена ли RPMB.
   Отделяет «RPMB не готова» от «cmnlib64/реентрантность».
2. **Реентрантность фаза 2** — сверить наш хук (дропать app_access_lock при
   обслуживании listener).
3. **Поднять полный набор listener'ов + backing ДО INIT** (как qseecomd):
   GP-FS/SFS(10) + RPMB(0x2000) + каталоги /data/vendor/tzstorage,
   /mnt/vendor/persist/data — и реально ОБСЛУЖИВАТЬ их.
4. **rmt_storage** — backing TZ storage; возможно RPMB нуждается в нём.

Firmware .mbn (goodixfp/cmnlib/keymaster) в git НЕТ (в modem/tz разделах) —
брать из образа OP8 SM8250 при нужде сверки версий TA.

## ⭐⭐⭐ WORKFLOW-СИНТЕЗ (2 сент): два решающих открытия

### Открытие 1 — патч anti-rollback бил в МЁРТВЫЙ код
goodixfp.bin @0x337d3c = `02 00 00 00` → **g_file_system_type == 2** → все
файловые операции идут через **qsee_fts**, НЕ qsee_sfs. Гейт
`version_Is_Rpmb_Usage_Enabled@0x22a74` (и qsee_sfs_is_anti_rollback@0x2a25c)
стоит ТОЛЬКО на type-3/sfs пути — он НЕДОСТИЖИМ. ⇒ вот почему патч 0x22a74/
0x2a25c НИЧЕГО не дал. fts достигает version_*_Rpmb блока минуя гейт.

### Открытие 2 — HLOS-реентрантность BLOCKED сломана
Устройство фаза-2 (`qcom,qsee-reentrancy-support=<2>` ✅дв live.dts:2687).
Вложенный storage-listener всплывает как **BLOCKED_ON_LISTENER(2)**, который
downstream обслуживает **session-keyed CONTINUE_BLOCKED_REQUEST = 0x32000207**
(args[0]=resp_type=session, БЕЗ handler/буфера; qseecom.c:2344/2384/3622). Наш
continue отвечает не тем SMC → TZ блокирован навсегда → watchdog. INCOMPLETE-
only (app_start/commonlib) работал — оттого дефект прятался до вложенного storage.

### МЁРТВЫЕ пути (закрыты доказательно)
* Патч 0x22a74/0x2a25c (sfs-гейт) — мёртвый код (type=2).
* CLK_QSEE — `qcom,no-clock-support` (live.dts:2689), no-op.
* cmnlib_oem_config.xml — файла нет (у ODM это gpfspath), запечено.
* Ре-провижн RPMB — уже провижинена, необратимо.

### Ранжир решений
* **S0** (инструментировать continue-loop, лог result/resp_type/data, 1 прогон
  INIT) — диагностика, различает 3 гипотезы разом.
* **S1** BLOCKED→0x32000207 session-continue — HLOS-корень, high conf.
* **S2** зарегистрировать TIME(11)+SSD(0x3000) — необходимое условие.
* **S4** патч cmnlib **fts** `0x1a9f0`→ret0 (не sfs-гейт) — обход.
* **S5** флип goodixfp байт @0x337d3c **2→1** → plain FS(10), минуя cmnlib/fts/
  RPMB; шаблоны обычными файлами. Риск: TA может проверять подпись при загрузке.
* **S6** TZ diag через `qcom_scm_io_readl` (не ioremap!) — TZ читает в EL3, обходя
  secure OCIMEM; reset-risk НОЛЬ. **S7** SDI ramdump → EL3 fault PC.

### Первый БЕЗОПАСНЫЙ шаг (из синтеза)
1. **UFS RPMB counter probe** (/dev/bsg/0:0:0:49476, READ_WRITE_COUNTER, SG_IO
   SPO 0xB5/SPI 0xA2): result==0 → провижинена И HLOS↔RPMB работает. НОЛЬ риска.
2. ⚠️ НЕ слать TZ_OS_RPMB_CHECK_PROV_STATUS (0x32000403) первым — если listener
   сломан, тот SMC сам BLOCK'нется → сброс. Безопасен только ПОСЛЕ S1.
3. Наше наблюдение (нет SPINEL-RE, handler не зван) = 3-я ветвь S0: cmnlib падает
   в fts ДО возврата BLOCKED/INCOMPLETE → Tier-2 (S4/S5). Но возможна гонка лога.

## 🎉🎉🎉 ОРЕХ РАСКОЛОТ (2 сент): реентрантность починена, INIT РАБОТАЕТ

Исправление цикла реентрантности по downstream (workflow-синтез Открытие 2)
СРАБОТАЛО ✅зам:
* **INIT (1001) rc=0, INIT_FINISHED (1005) rc=0, планшет ЖИВ** (uptime 45→50).
* Листенер 10 (FS) РЕАЛЬНО обслуживается: `SPINEL-RE-LSTNR: SERVICE id=10 (FS)`.
* Демон разбужен, получает НАСТОЯЩИЕ файловые запросы (op=0x0202 = FILE_OPEN):
  `/data/vendor_de/0/fpdata/gf_nav_base.so`, `gf_fdt_base.so`, `aspm*`,
  `/persist/data/fingerprint_*`. Ровно пути из gpfspath (ODM-разведка).
* Поток: INIT → INCOMPLETE(data=10) повторно → hook обслуживает (handler→демон,
  FILE_OPEN) → RESPONSE_HANDLER(0x32000203) → resp_type=0xee01 «готово» →
  result=0 → INIT завершается.

### Что было корнем (НЕ RPMB, НЕ cmnlib-патч)
Сломанное HLOS-рукопожатие реентрантности:
* INCOMPLETE(data=lstnr) надо ОБСЛУЖИТЬ (handler пишет ответ в SB) и слать
  RESPONSE_HANDLER 0x32000203 — мы слали пустой incomplete_continue БЕЗ
  обслуживания.
* (BLOCKED надо слать continue_blocked 0x32000207 — тоже починено, но storage
  идёт через INCOMPLETE, не BLOCKED).
* handler теперь возвращает -ENODEV для чужих id → generic-ветвь для commonlib.

Демон-заглушка отвечает ENOENT (файлов нет — первый запуск), goodixfp это
принимает штатно, INIT rc=0.

### Осталось до рабочего отпечатка
1. **Демон: реальные файловые операции** (open/read/write/mkdir по fs_msg.h,
   порт minkipc fs.c) → шаблоны/калибровка ПЕРСИСТЯТ в /data/vendor_de/0/fpdata.
2. Каталоги: /data/vendor_de/0/fpdata, /mnt/vendor/persist/data (0770).
3. Enroll (1060/g3) → verify (1061) в RAM, затем с хранением.

⚠️ Урок: два независимых открытия workflow (мёртвый sfs-патч + сломанный
BLOCKED/INCOMPLETE continue) вместе раскрыли корень. Сток (OnePlus SM8250) +
downstream qseecom.c = ключ.

## ✅ Файловая служба-листенер РАБОТАЕТ (2 сент)

Демон spinel-lstnrd переписан на полный протокол fs (minkipc): open/creat/read/
write/close/lseek/mkdir/testdir/unlink/rmdir/rename/fstat/lstat/sync/get_errno.
Реальные ОС-дескрипторы; пути TZ ремапятся под БАЗУ /root/spinel-fp (наш /data =
vfat загрузчика); родители создаются авто. Ответ пишется в ТОТ ЖЕ SB, ioctl DONE.

Гонка задвоения (демон опрашивал повторно до сброса req_pending) исправлена:
ioctl DONE снимает req_pending ДО complete().

Результат ✅зам: goodixfp INIT делает полное защищённое хранилище через листенер:
* OPEN gf_calibration/gf_nav_base/gf_fdt_base/gf_otp_info/asp_last_asplevel (−1
  первый раз, штатно), OPEN+WRITE fingerprint_version (создан, персист);
* пути чистые (битых 0), INIT/INIT_FINISHED rc=0, планшет жив.

Осталось: SET_ACTIVE_GROUP(1014) → enroll(1060/g3, по прерываниям) → POST_ENROLL
(1008) → SAVE(1012) → verify(1061). Шаблоны теперь ПЕРСИСТЯТ.

## ✅ Полная цепочка запуска + armed-enroll через хранилище (2 сент)

proba polnyj=1 zapis=1: `100→1000→1001→1004(DOWNLOAD_CFG)→1005→1014(SET_ACTIVE_
GROUP)→g3/1060(TEST_UNTRUSTED_ENROLL armed)` — ВСЕ rc=0, планшет жив ✅зам.

**SET_ACTIVE_GROUP (1014) раскрыл схему хранилища** — goodixfp через листенер
перечислил слоты группы 0 (все OPEN, -1 первый раз):
* `finger_0_0.so`..`finger_0_4.so` (+`_bak`) — 5 слотов образцов пальца 0;
* `auth_token.so`/`auth_token_0.so`, `backup_finger_0.so`,
  `gf_module_info_0.so`, `gf_asp_version_info_0.so` — токен/метаданные;
* калибровка gf_calibration/gf_nav_base/gf_fdt_base/gf_otp_info.
Ничего нет → «пальцев не записано» → встаёт в режим записи (1060 armed).

### Осталось: физическая запись пальца
1. IRQ пальца (request_irq 303 давал -EINVAL — разобрать).
2. Цикл записи: 1060 armed → по каждому прерыванию GF_CMD_IRQ(1016) → накопление
   → POST_ENROLL(1008) → SAVE(1012) пишет finger_0_0.so (персист через листенер).
3. verify: 1061.
4. notify-send «приложи палец» для касаний.

Хранилище шаблонов ПОЛНОСТЬЮ готово принять запись.

## Запись пальца: захват налетел на TZ-сброс (2 сент)

Интерактивный enroll: IRQ РАБОТАЕТ (касания считаются ✅зам), цепочка armed
(1060)→…→POST_ENROLL(1008)→SAVE(1012) проходит. НО мой цикл захвата
(GF_CMD_IRQ×8 на каждое касание) уронил TZ — известная ловушка
[[spinel-gf-lavina-preryvanij]]: вызов TZ на КАЖДЫЙ край роняет планшет.
Хранилище пережило сброс (fingerprint_version персист).

### Безопасный путь (сперва сток, БЕЗ железных тестов)
Прочесть точный протокол записи-по-прерыванию в стоке:
* RAZBOR §5 «gf_irq_enroll», goodixfp bin: сколько раз и КАКАЯ команда на
  касание при записи (не GF_CMD_IRQ×8 вслепую); машина состояний записи;
  нужен ли фон 1075 [[spinel-gf-preryvanie-rabotaet]]; ограничитель частоты
  [[spinel-gf-lavina-preryvanij]].
* Затем — rate-limited безопасный цикл записи, одно касание = один выверенный
  вызов, с проверкой состояния.

### Статус направления
ОРЕХ (TZ-хранилище) РАСКОЛОТ и работает. IRQ работает. Осталась ТОЛЬКО
аккуратная машина записи образцов (тюнинг по стоку + интерактив), не
принципиальная стена.

## Протокол записи из стока — синтез (2 сент)

### Цепочка (RAZBOR §5)
```
пуск:    1000 → 1001 → 1004 → 1005 → 1014
запись:  1060 (гр.3, тело 96) → ПО КАЖДОМУ прерыванию GF_CMD_IRQ → 1008 → 1012
```
gf_irq_enroll @0x829c0 накапливает образец по каждому краю; сторожевой таймер
обрывает затянувшуюся запись.

### Машина состояний прерывания (тело+0x4FF9C)
* Заполнить блок управления ОДИН раз (0x4FF9C=1) перед первым GF_CMD_IRQ.
* Повторять GF_CMD_IRQ пока шаг = 4 (GET_IMAGE) или 5 (POST_GET_IMAGE), до 3 раз.
* Шаги: PRE_PROCESS/PROCESS/PRE_GET_IMAGE(3)/GET_IMAGE(4)/POST(5)/CLEAR_IRQ(6)/
  7=выход.

### ⚠️ ПОЧЕМУ КРАШНУЛОСЬ: неверная длина тела GF_CMD_IRQ
**GF_CMD_IRQ (1016) требует тело 0x4FFA8 = 327592 байт**, а мой цикл слал
дефолтные 96 → падение. «У каждой команды своя длина»: 1016=0x4FFA8,
1110=0x73E8, 1010=0x78, прочие=0x60. Плюс НЕ звать из обработчика прерывания
(мой цикл — из контекста insmod, это ок).

### ⚠️ НАСТОЯЩИЙ БАРЬЕР: обнаружение пальца (fdt_delta/OTP)
* Съёмка начинается только если кристалл поднял разряды 0x8080 (gf_irq @0x81ddc).
* Их поднимает ОБНАРУЖЕНИЕ пальца, а оно требует порога **fdt_delta** из
  **OTP-калибровки** кристалла.
* Кристалл поднимает 0x2 в NAV/KEY, но МОЛЧИТ в MODE_FF/enroll → похоже
  fdt_delta=0 (OTP не прочитан/не вычислен). GF_ERROR_GET_OTP_INFO_FAILED(1067).
* Прошивка имеет base_get_fdt_base, base_check_fdt_base_validation,
  gf_base_update_finger_base. gxfp5130: калибровка выводится из OTP (tcode,
  fdt_delta, 4 ЦАП).

### НОВЫЙ угол (хранилище ТЕПЕРЬ работает)
Прошлый разбор был БЕЗ листенера. Теперь INIT читает gf_calibration.so /
gf_fdt_base.so / gf_otp_info.so (видели OPEN, все -1). На стоке они хранят
калибровку. Прошивка при первом запуске должна ВЫЧИСЛИТЬ из OTP и ЗАПИСАТЬ их —
но мы видели ТОЛЬКО запись fingerprint_version, калибровка НЕ записана ⇒
OTP-калибровка не сработала при INIT.

### Следующий шаг (safe, сток)
Найти КОМАНДУ калибровки, читающую OTP → fdt_delta → пишущую gf_calibration/
gf_fdt_base (теперь персист возможен). Кандидаты: TEST_* (1024-1030),
gf_base_update_finger_base. Разобрать в goodixfp, что запускает OTP-калибровку.
Затем: (1) исправить длину 1016=0x4FFA8, (2) триггернуть калибровку, (3) enroll.

## OTP-калибровка (BIO_CALIBRATION 1064) — разбор (2 сент)

### Команда найдена
GF_CMD_TEST_BIO_CALIBRATION = **1064** (группа 3, testcmd 38, тело 96).
Функция gf_cali_common_calibration (~0x5ad5c) → gf_base_update_finger_base →
base_save_fdt_base пишет gf_fdt_base.so/gf_calibration.so. OTP: read_otp
@0x36ed57 / gf_hw_get_otp_data (default_dac[0..3], tcode).

### ✅ ВАЖНО: тест-команды БОЛЬШЕ НЕ КРАШАТ
1064 rc=0, планшет жив ⇒ прошлые краши тест/калибровочных команд были той самой
реентрантностью (починена). Весь тест-простор (1024-1096) теперь доступен.

### Но калибровка не записала базу
1064 читал gf_otp_info/gf_calibration/gf_fdt_base (все -1), НЕ записал.
Причина по строкам goodixfp:
* `invalid g_calibration_data.calibration_type=%d` — нужен calibration_type в
  теле (я слал 0 → invalid → noop → rc=0 без работы);
* `calibration_params->frameNum`, `image_quality`, `valid_area` — калибровка
  ЗАХВАТЫВАЕТ КАДРЫ и считает базу из них (не чисто OTP);
* algo_calibration_decode_{frame_num,kr,b,sensor_id} — декод параметров.

### Барьер: заводская калибровка fdt-базы
Обнаружение пальца нужен fdt_base; его создаёт калибровка захватом опорных
кадров + OTP. Нужно: (1) верный calibration_type + frameNum + параметры в теле
1064, (2) сырой захват кадра (raw, без обнаружения пальца) работает. Это тот же
барьер, что и у прошлой сессии («нет опорного снимка»), но теперь: 1064 не
крашит, хранилище пишет базу — не хватает ПАРАМЕТРОВ калибровки и сырого кадра.

### Следующий заход
Разобрать тело калибровки: значения calibration_type (BIO), frameNum, откуда
параметры (OTP-декод vs blob); как идёт сырой захват кадра при калибровке
(режим, без fdt-порога). Возможно gxfp5130 даст форму (otp_calibration.rs).

## ⚠️ ПОПРАВКА: 1064 КРАШИТ С ПЕРЕБОЯМИ (2 сент)

Проверка uptime до/после в двух прогонах подряд (Alex настоял):
* прогон 1: uptime 2957→2962 (+5s) — ЖИВ ✅;
* прогон 2: **СБРОС** (uptime после = 66s, планшет перезагрузился).

⇒ **«тест-команды больше не крашат» — НЕВЕРНО/преждевременно.** BIO_CALIBRATION
(1064) крашит С ПЕРЕБОЯМИ. Реентрантность решила вис INIT/storage, но
калибровочный путь (захват КАДРОВ / SPI в TZ) ещё нестабилен — второй прогон
роняет. Урок [[spinel-uptime-ne-zavisanie]]: мерить uptime до/после в ОДНОМ
прогоне и не заявлять «стабильно» по одному успеху.

Барьер калибровки fdt-базы стоит + сама 1064 нестабильна (перебойный сброс на
пути захвата кадра). Разбирать причину нестабильности захвата ОСТОРОЖНО, по
одному прогону, с uptime-проверкой.

## ✅ НЕСТАБИЛЬНОСТЬ РАЗОБРАНА (3 сент): виновата ре-регистрация листенера

Изоляция переменной (uptime до/после каждого прогона):
* polnyj БЕЗ 1064, листенер rmmod/insmod между прогонами: прогон1 жив,
  прогон2 СБРОС.
* Держим листенер загруженным, перезагружаем ТОЛЬКО proba:
  - polnyj ×3: ВСЕ ЖИВЫ ✅;
  - polnyj+kalibr(1064) ×3: ВСЕ ЖИВЫ ✅.

⇒ **Крах — от ре-регистрации листенера** (rmmod spinel_qsee_listener →
unregister + free SB/пул → TZ держит ссылку на освобождённый SB → сброс на
СЛЕДУЮЩЕЙ сессии). Storage-путь, калибровка (1064) и захват — СТАБИЛЬНЫ, когда
листенер загружен ОДИН раз.

### Поправка к «1064 крашит с перебоями»
НЕВЕРНО — 1064 НЕ крашит. Крашила ре-регистрация листенера в тест-цикле. С
листенером-службой (один insmod) 1064 стабильна.

### Следствия
1. Листенер — ПОСТОЯННАЯ служба (грузить один раз), НЕ перезагружать между
   сессиями. Реальное развёртывание так и работает.
2. (Опц., для надёжности) починить unregister листенера: чистить TZ-ссылку на
   SB перед free пула, ЛИБО переиспользовать SB между ре-регистрациями.
3. Калибровку/захват можно исследовать БЕЗОПАСНО при постоянном листенере.

### Калибровка 1064 всё ещё noop
Стабильна, но базу не пишет (calibration_type=0 → invalid). Нужны параметры тела
(calibration_type, frameNum) — детерминированно, БЕЗ риска сброса при постоянном
листенере.

## ✅ ФИКС unregister листенера (3 сент)

Механизм краха подтверждён (no_free=1 → 3 сессии живы): освобождение SB
(pool_free → удаление SHM-моста) роняло следующую сессию, т.к. cmnlib в TZ
кеширует адрес SB с первой регистрации.

**Правильный фикс:** листенер берёт SB из ПОСТОЯННОГО пула qcom_scm
(`qcom_scm_get_tzmem_pool`, экспортирован), а не создаёт свой. Пул и его SHM-мост
живут всё время работы ядра → мост не удаляется на выгрузке модуля → ре-
регистрация безопасна. На выгрузке — только `qcom_tzmem_free` буферов.

Проверено ✅зам: 3 сессии с rmmod/insmod модуля листенера — все живы, storage
работает (fingerprint_version создаётся и читается). no_free-хак убран.

## Тело калибровки (3 сент) — разбор

BIO_CALIBRATION (1064) внутри:
* `g_calibration_data.calibration_type` — обязателен (0 → «invalid», noop);
* тип ставит `gf_cali_set_calibration_type`/`gf_cali_common_set_calibration_type`;
* `image_quality`, `valid_area` — считаются из КАДРА (нужен захват);
* `calibration_params` (frameNum, kr, b, sensor_id) — отдельная структура,
  декодируется algo_calibration_decode_*; «out of memory for calibration_params»;
* enroll: `g_config.continue_frame_num = 2` (2 непрерывных кадра).

### Почему 1064 с нулевым телом = noop
calibration_type остался 0 (не установлен) → «invalid calibration_type» →
калибровка не пошла, база не записана. Нужно либо тип в теле, либо
предварительная команда set_calibration_type, либо calibration_params из
конфигурационного блоба (заводского).

### Барьер = заводская калибровка (тип + параметры + кадр)
Это фронтир: нужны calibration_type (валидное значение), calibration_params
(frameNum/kr/b — откуда?), и захват опорного кадра. gxfp5130 (otp_calibration.rs)
показывает: fdt_delta ВЫВОДИМ из OTP (tcode, 4 ЦАП) — возможно есть путь короче
полной BIO-калибровки (base_get_fdt_base/gf_base_update_finger_base из OTP).

### Следующее (теперь БЕЗОПАСНО — листенер стабилен)
1. Эмпирически: слать 1064 с calibration_type в теле (тело+96?), перебрать
   валидные значения — какое запускает калибровку (пишет базу).
2. ЛИБО read-only: разобрать gf_cali_set_calibration_type (BL-вызовы, значение
   аргумента у BIO) + gf_base_update_finger_base (нужен ли кадр или чистый OTP).

## Эмпирика calibration_type (3 сент) — отрицательный, но сузил

Перебор calibration_type 0..12 в тело+96, 1064 группа 3:
* ВСЕ type: rc=0, itog=0, база НЕ записана (только fingerprint_version);
* планшет жив весь перебор (415→421) — фикс листенера держит, безопасно ✅зам.

⇒ тело+96 — НЕ место для calibration_type, ЛИБО калибровке мало типа: нужны
`calibration_params` (frameNum/kr/b — из конфиг-блоба gf_calibration.so, которого
нет) И/ИЛИ захват опорного кадра. Не простое значение.

### Осталось (калибровка = глубокий фронтир)
1. Read-only: разобрать ТОЧНО обработчик 1064 — откуда читает calibration_type
   (какое смещение тела) и calibration_params; вызов gf_cali_set_calibration_type
   (значение аргумента); нужен ли кадр или чистый OTP (gf_base_update_finger_base).
2. gxfp5130 otp_calibration.rs — форма вывода fdt_delta из OTP (короткий путь?).
3. Возможно нужен заводской gf_calibration.so (config-блоб) — взять из образа OP8.

### СТАТУС СЕССИИ (итог)
РАСКОЛОТО и СТАБИЛЬНО: TZ-хранилище шаблонов (реентрантность 0x32000203/0x32000207
+ fs-листенер + персист + фикс ре-регистрации через постоянный пул qcom_scm).
Полная цепочка 1000-1001-1004-1005-1014 + armed-enroll + SET_ACTIVE_GROUP
(слоты finger_0_0..4). Тест-команды доступны (краши были реентрантностью/
ре-регистрацией). IRQ работает.
ФРОНТИР: калибровка fdt-базы (обнаружение пальца) — заводская, нужны параметры +
кадр; enroll-съёмка ждёт fdt-базы.

## gxfp5130 OTP-калибровка — ФОРМА (3 сент, разведчик)

Тот же Milan, читаемый Rust. Ключевое (форма портируема, числа — нет):

### Порог fdt_delta — ЧИСТО из OTP (кадр НЕ нужен)
* READ_OTP = cmd 0xA6, тело [0,0], возвращает всю страницу OTP.
* tcode/fdt_delta — ЗАМКНУТАЯ формула от байт OTP (otp_layout.rs:47-50):
  `tcode=((rt>>4)+1)*0x10+0x40`; `fdt_delta=(((rt&0xf)+2)*100*0x100/tcode/3)>>4`,
  rt — байт-носитель tcode с проверкой доп.кода. Defaults tcode=0x80, fdt_delta=0x15.
* DAC: dac0=(otp[base]<<4)|8, dac1..3 raw; base=0x32(FT)/0x2e(MT).

### НО «палец вниз» ≠ только порог — нужны ещё 2 шага
1. **Записать DAC в банк 0x2xx** через REG_WRITE=0x80 (un-rail фронтенда) —
   иначе скан возвращает насыщенный placeholder 0x01ff, палец не срабатывает.
   Регистры: tcode 0x5c, fdt_delta<<8 0x82, DAC 0x220/0x236/0x238/0x23a.
2. **Активный FDT-скан** FDT_MODE=0x36 → валидный базовый кадр (не насыщенный,
   retry ×8), кодировка зон (sample>>1<<8)|0x80. Это БАЗА (не биометр. кадр,
   пальца НЕ надо — чистый сенсор). Отказ = placeholder 0x01ff, тот же
   gfCheckFDTBase, что и наш base_check_fdt_base_validation.

### Для нашего трастлета
Трастлет goodixfp владеет SPI и делает это САМ в калибровке (не надо
реплицировать математику). Нужно ТРИГГЕРНУТЬ его калибровку: read OTP → write
DAC (un-rail) → FDT-скан (база, БЕЗ пальца) → save. Наш «нет 0x8080 в enroll» =
ровно railed-фронтенд (DAC не записаны) → placeholder. ⇒ BIO_CALIBRATION должна:
прочитать OTP, записать DAC, сделать FDT-скан базы, сохранить. Кадр базы —
БЕЗ пальца. Ждём дизасм 1064 (что именно она делает/чего ей не хватает).

Команды Milan (могут совпасть с GF9558): 0xA6 read_otp, 0x80 reg_write/0xB0 ack,
0x82 reg_read, 0x90 upload_cfg, 0x32/0x34/0x36 fdt down/up/mode, 0xDA fdt_status.

## OP8/блоб калибровки (3 сент, разведчик) — блоба НЕТ, но подтверждает путь

* **Заводского config-блоба калибровки НЕТ** нигде в vendor/odm. gf_calibration.so
  / gf_fdt_base.so — ТОЛЬКО runtime-выход в /data/vendor_de/0/fpdata/, никогда не
  в образе. Калибровка per-die (OTP различается по кристаллу) — копировать нельзя.
* gpfspath_oem_config.xml — red herring (только пути, без frameNum/kr/b/type).
* Параметры калибровки (frameNum, calibration_type, kr, b) ВШИТЫ в HAL/трастлет,
  не в файле.
* ⇒ калибровку надо ВЫЧИСЛИТЬ на устройстве из OTP + кадры (runtime-путь), НЕ
  копировать. Это подтверждает: триггерить BIO_CALIBRATION трастлета.
* ODM подтверждён снова: fingerprint.20801_goodix9558_G3.default.so +
  libgf_ud_hal_20801_G3_oplus.so — байт-в-байт с OP8 (если нужна чистая копия HAL
  — брать из OP8 dump, TheMuppets/AICP odm/lib64).

### Сходимость 2+3
Калибровка = read OTP → write DAC (un-rail) → FDT-скан базы (БЕЗ пальца) → save,
всё внутри трастлета. Блоба нет — только триггер. Ждём дизасм 1064 (как
триггернуть: смещение type/params, делает ли она DAC-write+скан).

## Дизасм 1064 (3 сент, разведчик) — калибровка КАДРО-УПРАВЛЯЕМА

### Почему мой перебор был мимо
* `calibration_type` — НЕ поле тела, а ГЛОБАЛ ([0x724008]/[0x724370]).
* тело+96/+100 = поле OPERATION, прошивка сама ставит его из command_id
  (1064→operation 38, `0x22160`: `[x8+608]`==1064 → `mov w0,#0x26`). Мой перебор
  0..12 писал мусорные operation вне диапазона 24..62 → no-op → itog=0.
* 1064 ОДНА не запускает калибровку: только ВЗВОДИТ operation/режим; сама
  калибровка идёт ПОЗЖЕ на захваченных фоновых кадрах в IRQ (gf_irq_process_test
  @0x86aac, jump-table по offset +100).

### Что реально нужно (мин. последовательность)
1. Питание + настройка сенсора DAC из OTP (gf_hw_get_otp_data), режим bio-cali.
2. **calibration_type = 1** (глобал; ставит gf_cali_set_calibration_type @0x5a790)
   — единственное валидное для finger-base пути (проверки @0x58e8c, @0x5afc0).
3. **CRC-валидный calibration_params блоб** (frameNum/kr/b/sensor_id) загружен
   (gf_cali_common_load, algo_check_crc32). НЕ из тела, НЕ из OTP.
4. **Захват фонового кадра** (БЕЗ пальца) — gf_cali_preprocess @0x58dac берёт
   буфер кадра, algo preProcess @0xf8700 → image_quality/valid_area (ВЫХОДЫ).
5. gf_base_update_finger_base @0x4dd0c (читает FDT-базу сенсора milan_read_fdt_base
   @0x9a058 через secspi + gf_cali_calibration на кадре) → gf_base_save_calibration
   @0x4c1a8 пишет базу.

### ⚠️ fdt_base — НЕ чисто из OTP
Вопреки gxfp5130-форме, у нашего трастлета база = f(фоновый кадр + FDT-регистры
сенсора). OTP (default_dac, tcode) только НАСТРАИВАЕТ фронтенд под захват
(проверка tcode∈(0,320], diff_from_otp∈[300,1700]). Chicken-and-egg: нужен
рабочий захват фонового кадра, чтобы пересоздать базу.

### СХОДИМОСТЬ всех трёх → корень
Всё упирается в ЗАХВАТ ФОНОВОГО КАДРА (без пальца). gxfp5130: фронтенд railed
(DAC не записаны) → скан = насыщенный 0x01ff → база отвергается. Наш «нет 0x8080»
= ровно это. ⇒ настоящий фронтир = заставить сенсор выдать валидный фоновый кадр:
записать DAC из OTP (un-rail) → FDT-скан → база. calibration_type=1 + params-блоб
+ armed capture — обвязка; сердце — валидный фоновый кадр.

### Следующий шаг
Разобрать в трастлете: (а) какая команда/путь ставит calibration_type=1 и грузит
params-блоб (откуда он на ПЕРВОМ запуске — вшит?); (б) как взводится ЗАХВАТ
фонового кадра и почему сейчас кадр насыщенный/пустой (DAC записаны? gf_hw_get_
otp_data успешен? milan_read_fdt_base что возвращает). Это узел, где сходятся
enroll-съёмка и калибровка.

## Путь захвата фонового кадра — разбор (3 сент, разведчик)

Глобалы: g_milan_chip_handle @0x69c510 (OTP-кэш +0xa0, cal_dac +0xf4);
g_calibration_data @0x724368 (calibration_type +8 = 0x724370).

### DAC un-rail СУЩЕСТВУЕТ в трастлете
`milan_update_dac_value` @0x93fb4 пишет (reg_access dir=1) в банк 0x2xx:
* 0x220 = (old&0xe00f)|(default_dac[0]<<4)  ⇒ младший ниббл ДОЛЖЕН читаться 8;
* 0x236=default_dac[1], 0x238=default_dac[2], 0x23a=default_dac[3];
* источник — cal_dac @0x69c510+0xf4 (memcpy из OTP-блока 0x69c5b0).
Зовётся на **cmd 1000 (INIT)** и **cmd 1005** (через gf_hw_chip_config_init) и на
входе IMAGE-режима. НЕ на 1014 и НЕ на 1060/1061 (тот вообще НЕ диспатчится TA —
это RAM-обход). tcode/fdt_delta не отдельные регистры — едут в FDT-конфиг-блоке.

### FDT-скан фонового кадра ЖЁСТКО заперт на валидности DAC
`milan_read_fdt_base` @0x9a058: 0xc0→config-upload→0xc1→**0xa1 (скан)**→0xc0→
0xa2 (чтение 24Б). Гейт @0x9a1a8: если check_sensor_dac_validity != 0 →
«check sensor dac invalid» → **АБОРТ до скана** → база остаётся насыщенным
placeholder (~0x1ff, младшие 12 бит, сдвиг по reg0x58 bit8).

### check_sensor_dac_validity @0x95200 — ТОЛЬКО ЧТЕНИЕ
Читает регистры 0x220/0x236/0x238/0x23a и сверяет с OTP-кэшем +0xf4/f6/f8/fa.
Валидно ⇔ 0x220 младший ниббл == 8 И старшие == кэш И 0x236/8/a == кэш. Иначе
«sensor dac is invalidity, need reset» — reset = перезапустить milan_update_dac_value.

### calibration_type=1 и params-блоб — CHICKEN-AND-EGG
* type=1 ставится ТОЛЬКО при успешной загрузке blob (gf_cali_common_load @0x5c304).
  Отдельные сеттеры gf_cali_set_calibration_type — МЁРТВЫЙ код (нет вызовов).
* **Вшитого default-блоба НЕТ.** Первый запуск: нет gf_calibration.so →
  gf_cali_load ПРОПУЩЕН целиком, из OTP ничего не синтезируется, type=0.
* params (frameNum/kr/b) — ТОЛЬКО из ранее сохранённого gf_calibration.so
  (0xF0-magic + CRC32), который пишет сама калибровка. На стоке первый blob даёт
  заводской GRCalibration (PC-тул) — у нас его нет.

### ⚠️ РАЗДЕЛЕНИЕ: детект пальца ≠ блоб
DAC un-rail и FDT-база (детект пальца down/up) НЕ требуют blob — только OTP DAC.
Blob нужен для БИОМЕТРИЧЕСКОГО алго (обработка образа). ⇒ finger-DETECT может
заработать с одними DAC+скан, а enroll-съёмка/сопоставление — уже с blob.

### Узкий тестируемый вывод
un-rail есть, зовётся на 1000/1005. Значит railed-фронтенд ⇒ ЛИБО OTP
default_dac(+0xf4) = 0 (нулевой cal_dac → пишем нули → validity падает), ЛИБО
запись не доходит до сенсора. **ЭТО ТЕСТИРУЕМО** (листенер стабилен): после
1000+1005 прочитать регистры 0x220/0x236/0x238/0x23a (proba 1035/spi_adres) и
сверить — записаны ли (ниббл 8, ненули). Прочитать OTP default_dac.

## Чтение DAC-регистров — неинформативно + ДВЕ стены (3 сент)

Чтение 0x00/0x02/0x58/0x82/0x220.. через 1059 → ВСЕ 0x0000 (включая chip-id).
⇒ 1059-read НЕ доходит до этих регистров (сенсор без клока/неактивен при чтении,
ЛИБО читает иное пространство/смещение). Состояние DAC так не проверить.

### Стена 1: railed фронтенд (DAC/OTP)
un-rail (milan_update_dac_value) есть на 1000/1005, но эффективен лишь если OTP
default_dac(+0xf4) ненулевой И запись доходит. Кадр насыщен ⇒ что-то из этого не
так. Проверить прямо (register-read) не вышло. Нужен другой способ увидеть
default_dac / состояние DAC (дамп OTP-инфо командой, либо сырой кадр test).

### Стена 2: bootstrap params-блоба (chicken-and-egg)
Биометрическая калибровка (и type=1) требует gf_calibration.so (0xF0+CRC32,
frameNum/kr/b). Его НЕТ: не в vendor/odm, не вшит в трастлет, на стоке первый даёт
заводской GRCalibration (PC-тул). ⇒ даже с рабочим детектом пальца enroll не
построит шаблон без params-блоба.

### Возможный путь bootstrap (глубокий)
Параметры МОДЕЛИ (не per-die) вшиты в HAL `libgf_ud_hal_20801_G3_oplus.so`
(userspace, есть у нас/OP8 байт-в-байт). Извлечь оттуда calibration_params,
собрать gf_calibration.so (0xF0+CRC32) → скормить трастлету. Крупная работа.

### ЧЕСТНЫЙ ИТОГ enroll-направления
Механизм КАРТИРОВАН ПОЛНОСТЬЮ (DAC un-rail, FDT-скан 0xc0/0xc1/0xa1/0xa2, гейт
валидности, type=1 из блоба, bootstrap). Но живой отпечаток упирается в ДВЕ стены:
(1) railed фронтенд — почему DAC не действуют (OTP default_dac? запись?);
(2) bootstrap заводского params-блоба (извлечь из HAL). Обе — глубокие подпроекты.
РАСКОЛОТО И СТАБИЛЬНО остаётся TZ-ХРАНИЛИЩЕ (тяжёлая часть) + полная цепочка
запуска + фикс листенера — это надёжная веха.

## Стена 2: пути загрузки блобов + связь со стеной 1 (3 сент)

Демон поймал fs-OPEN трастлета на INIT (все O_RDONLY, все -1 = отсутствуют):
  gf_otp_info.so, gf_calibration.so, gf_fdt_base.so, gf_nav_base.so,
  gf_module_info_0.so, finger_0_*, auth_token* — база чиста.
Трастлет НИЧЕГО не пишет обратно на голом INIT (нет CREAT/WRITE) — калибровка
не генерится сама, только грузится.

КУДА класть сконструированный блоб:
  /root/spinel-fp/data/vendor_de/0/fpdata/gf_calibration.so (демон отдаёт прямо).

СВЯЗЬ СТЕН: gf_otp_info.so ОТСУТСТВУЕТ ⇒ OTP читается ЖИВЬЁМ с сенсора, оттуда
default_dac (un-rail, стена 1). Railed-кадр ⇒ либо живое OTP-чтение даёт нули,
либо FDT-скан заперт. Стена 1 (DAC) и стена 2 (tuning-блоб) сходятся на OTP.

Строка "[gf_base][%s] set default value for Kr and frameNum" @0x352abf (образ
плоский) — путь ВШИТЫХ дефолтов Kr/frameNum. Если он даёт валидную калибровку
без заводского блоба — стена 2 во многом падает. Два разведчика разбирают:
(1) условие+значения дефолтов и продолжается ли калибровка без блоба;
(2) точный байт-формат gf_calibration.so (0xF0+CRC32) для конструирования.

## ПРОРЫВ: стена 2 функционально падает (3 сент, разведчик 1)

Трастлет НЕ делает hard-abort без gf_calibration.so.
- base-init @0x4e55c; лог "set default value for Kr and frameNum" @0x4e6dc
  (x1=0x352abf, tag base_load_calibration 0x352a9a).
- Триггер дефолтов (w20!=0 @0x4e6c0): блоб не загрузился (loader 0xaf360) ЛИБО
  parse/validate 0x59df8 не прошёл (CRC32 / preprocess_version / invalid data).
- Дефолт пишет gf_cali_common_init_kr_and_framenum @0x5b2e8: memset базы в 0,
  затем width*height 16-бит = 0x2000 (movi v0.8h,#0x20,lsl#8; Q13=1.0, НЕ float!),
  frameNum-слот = 0 (обнулён).
- ГЛАВНОЕ: w0=2 @0x4e7c0 — НЕ возврат; общий хвост @0x4ebb8 всё равно зовёт
  base_update_all @0x4ca68 → gf_state_get_default_operation @0xb2dc8 →
  gf_state_execute_operation @0xb3ee0, и функция ВОЗВРАЩАЕТ его статус (w19).
  ⇒ база может строиться НА СЕБЕ без заводского блоба. Качество ниже (нет
  per-pixel заводской коррекции, ставится флаг "defaults-used"), но НЕ заперто.
- HAL libgf_hal.so передаёт frame_num/preprocess_version/Kr трастлету
  (dump_calibration_params, "frame_num": %d, %s_kr.csv), но сами ДЕФОЛТЫ — в
  трастлете (0x2000), не в HAL.

⚠️ НЮАНС: дефолтный frameNum=0. FDT-скан по 0 кадрам = пустая база даже при
исправных DAC. Вероятно frameNum надо ДОСЛАТЬ командой (1004 DOWNLOAD_CONFIG),
как делает HAL. Это связывает со стеной 1.

РАЗВОРОТ: заводской блоб не нужен. Реальный блокер живого отпечатка = стена 1
(railed фронтенд: OTP default_dac / запись DAC / frameNum=0). Всё сходится на
живом OTP-чтении и подаче frameNum.

## Стена 2 ЗАКРЫТА (3 сент, оба разведчика + ковалка)

Формат блоба вскрыт полностью → specs/own/GF-CALIBRATION-FORMAT.md.
Ковалка: tools/spinel-kovka-calibration.py (собирает валидный 128+4N блоб,
zlib-CRC32 big-endian, ppv "Preprocess_v_1.01.01"; самопроверка прошла).

Два runtime-неизвестных форжа: sensor_id (@0x689f24) и геометрия W×H (из слова
@0x689ef8: W=ubfx#14,#9; H=lsr#23). ОБА заполняются с сенсора при chip-init,
статически нули ⇒ live-значения (одинаковы для любого GF9558/G3).

ВЫВОД по стене 2 (тройной итог):
1. Заводской блоб НЕ нужен для базы — дефолтный путь (0x2000/frameNum=0) идёт
   в base_update_all сам.
2. frameNum>0 (если дефолтный 0 ломает FDT-скан) шлётся телом 1004 (метод HAL),
   форж не обязателен.
3. Форж готов для заводского качества позже; нужны лишь sensor_id+геометрия,
   а это те же live-чтения сенсора, что и стена 1.

СХОЖДЕНИЕ: всё живое упирается в стену 1 — live OTP-чтение (default_dac для
un-rail, sensor_id, геометрия, frameNum). Следующий шаг — стена 1.

## Стена 1: эксперимент BIO_CAL + переоценка «railed» (3 сент)

Со стабильным листенером впервые triggerнул калибровку эмпирически:
- init 1000-1001-1004-1005-1014 → все rc=0, БЕЗ краха (uptime +6).
- CMD_TEST_BIO_CALIBRATION(1064) пустое тело → app rc=0, но result=1070.
  1070 (0x42e) = "invalid parameters cmd_id_code=%u" из диспетчера chip-series
  (@0x19a7c/0x21698) — команда ОТВЕРГНУТА по параметрам, НЕ «калибровка не
  удалась». BIO_CALIBRATION — заводская (GRCalibration), не путь бутстрапа.
- Демон: базу трастлет НЕ писал (только 4 байта fingerprint_version). Файлов
  базы на диске нет.

ПЕРЕОЦЕНКА: «railed фронтенд» я вывел из статической логики в раннем разборе, а
НЕ из живого захвата. Эмпирически: secspi жив, OTP читается, сенсор отвечает
(память). Скорее всего FDT-база просто НЕ triggerится нашей последовательностью
(нужна иная команда/параметры), а не railed.

Разведчик ищет: код GF_CMD_MT_BASE и др.; какая команда→milan_read_fdt_base(0xa1)
и сохранение gf_fdt_base.so; гейт (почему голый 1005 не генерит базу); формат
тела 1004 (frameNum); рекомендацию последовательности для генерации базы без
пальца. milan_read_fdt_base @0x9a058, check_sensor_dac_validity @0x95200,
milan_update_dac_value @0x93fb4, handle @0x69c510+0xf4.

## Стена 1: MT_BASE подтверждает молчаливый DAC-гейт (3 сент)

MT_BASE(1110) после init: rc=0, itog=0 (УСПЕХ на верхнем уровне), но
gf_fdt_base.so НЕ пишется (демон видит только OPEN O_RDONLY -1). ⇒ внутренний
FDT-скан молча падает на check_sensor_dac_validity @0x95200, кадр не сохранён,
base_update_all глотает ошибку и возвращает 0. Точно как описал разведчик.

Карта пути (разведчик стены 1):
- cmd-таблица @0x47cb68; GF_CMD_MT_BASE=1110, INIT_FINISHED=1005.
- 1005→gf_base_init_finished@0x4e55c→base_update_all@0x4ca68(arg=1)→
  base_get_fdt_base@0x4d5b8→milan_read_fdt_base@0x9a058(0xa1)→
  check_sensor_dac_validity@0x95200 (ГЕЙТ). rc!=0 ⇒ count=0 ⇒
  base_save_fdt_base пусто ⇒ файла нет. Внешний rc=0 глотает.
- FDT-база — БЕЗ пальца (касание = ошибка "finger is touched when get base").
- Патчить трастлет (NOP гейта) НЕЛЬЗЯ: подписан, TZ проверит (в отличие от ICP).

Блокер честного прохода: валидные DAC. Идут из OTP default_dac (handle+0xf4)
через milan_update_dac_value@0x93fb4 на 1005, ВОЗМОЖНО требуют реального
config-блоба в 1004 (пустой у нас). Агент извлекает config из libgf_hal.so и
разбирает гейт: нужен ли config или достаточно ненулевого OTP default_dac.

## Стена 1: точный диагноз + граница TZ (3 сент)

Полный разбор гейта (агент config+dac):
- check_sensor_dac_validity @0x95200 (имя из строки 0x364e8a). Читает 4 регистра
  по SPI (0x220/0x236/0x238/0x23a) и сверяет с OTP-кэшем g_milan_chip_handle
  (0x69c510)+0xf4 (default_dac[0..3]). Успех (возврат 0): 0x220 младший ниббл==8,
  биты[4:12]==dd[0], 0x236==dd[1], 0x238==low16 dd[2], 0x23a==high16 dd[2].
  Провал → 0x453=1107 "dac value is error!". Гейт зовётся в milan_read_fdt_base
  @0x9a1a8, требует возврат 0.
- Даже НУЛЕВОЙ default_dac проходит (OTP-CRC-fail форсит 0x220→0x08, кэш 0 —
  самосогласованно). Валит 1107 ТОЛЬКО когда ЖИВЫЕ регистры != кэш: milan_update_
  dac_value не отработал / сенсор потерял DAC-состояние / SPI-чтение не совпало.
- milan_update_dac_value @0x93fb4 СОХРАНЯЕТ младший ниббл 0x220 (маска 0xe00f);
  ниббл=8 ставит OTP-init (строки "OTP CRC check failed, and update 0x0220
  register to 0x08" @0x369c8b).

ИСКЛЮЧЕНО как причина:
- config 1004: HAL шлёт 96 НУЛЕЙ (gf_hal_download_cfg @0x19e5c), register-config
  резидентен в TA. Наш пустой 1004 = сток. НЕ блокер.
- калибровочный блоб: стена 2, дефолт работает. НЕ блокер.
- frameNum=0: скан свой счётчик кадров. НЕ блокер.
- пропущенный листенер: MT_BASE запросил ТОЛЬКО id=10 (FS) ×55 + 3 BLOCKED,
  всё обслужено, 0 ENODEV. Железо сенсора трастлет держит внутри TZ.

Эмпирика:
- 1005 и MT_BASE(1110): rc=0/itog=0 (успех снаружи), база НЕ пишется — скан
  молча падает на гейте, base_update_all глотает.
- GET_DEV_INFO(1089): result=0, тело = СОФТ-версии (android9.0/qsee/версии/
  Preprocess_v_1.01.01 ✓ подтверждает ppv форжа); живого сенсор-чтения не видно.
- 1064/1094/1097 → 1070 (invalid params); 1103 → 1003 — заводские, ждут
  структуру параметров (GRCalibration), гейт-ошибку так не вскрыть.
- 1059 register-read даёт 0 для ВСЕХ регистров, вкл. chip-id — читает СПЯЩИЙ
  сенсор ([[spinel-gf-kristall-spit]]: шина off после взведения).

ГРАНИЦА: причина 1107 — внутри TZ (OTP/DAC/сон сенсора), SPI-такт TZ-owned, из
Linux не наблюдаема; трастлет подписан (NOP гейта нельзя). Единственный рычаг
Linux — GPIO питания/сброса сенсора и их ТАЙМИНГ относительно TZ-операций.
СЛЕД: сверить нашу последовательность power/reset сенсора со стоком
([[spinel-snachala-stok]], [[spinel-gf-datchik-podnyat]]) — возможно, сенсор
спит/сброшен, когда гейт читает DAC.

## Стена 1: power/reset против стока — провенанс + старт (3 сент)

ПРОВЕНАНС «стока» (перепроверено по вопросу пользователя, компактизация не потеряла):
- Сток = двоичное с ЭТОГО планшета: деревья из образов, ядро из его boot,
  kallsyms/строки из того двоичного, замер на железе (AGENTS.md 76-98).
- ЛЕГИТИМНЫЙ сток для power/reset:
  * current_work/fingerprint-ref/trees/stock/goodix-fp.dts (узел из образа):
    fp-gpio-reset=pin85(ctrl0x65), fp-gpio-irq=pin23, goodix_vdd-supply.
  * source_artifacts/dumps/tb371fc-stock/boot_unpacked/kernel (38МБ ARM64 Image
    из boot ЭТОГО планшета). ⚠️ file_offset == адрес kallsyms (подтверждено
    дизасмом 0xa709c8; строка "Goodixfp power_on in"@0x1b1c73f сверена побайтно).
  * current_work/fingerprint-ref/kernel/{imena,stroki}-otpechatka.txt — оттуда же.
- НЕ свидетельство: source_artifacts/source_codes/kernel-downstream/vmlinux
  (собрано из downstream-ИСХОДНИКОВ). source_artifacts вообще — только подсказка
  имён/мест ([[spinel-istochniki-ne-nashi]]).

Наш power/reset (spinel_fp_razbudit) уже стоко-производен: gf_power_on@0xa709c8,
gf_hw_reset@0xa70c48 — функции СТОКОВОГО ЯДРА планшета (диапазон 0xa70xxx рядом с
goodix_fb_state_chg_callback@0xa70708), НЕ трастлета. Агент сверяет точный тайминг
(особенно post-reset settle — время на POR/загрузку OTP в DAC-регистры).

## Стена 1: power/reset ИСКЛЮЧЁН измерением (3 сент, агент сверки)

Сверка нашей spinel_fp_razbudit со стоковыми gf_power_on@0xa709c8 /
gf_hw_reset@0xa70c48 (двоичное ядро планшета):
- Стоковая последовательность: reset LOW → 40мс → set_voltage(3.0В) → enable →
  reset HIGH → 10мс(+10мс ioctl); hw_reset: HIGH→LOW→3мс→HIGH→3мс settle.
- Единственное «отличие» — сток явно зовёт regulator_set_voltage(3.0В); мы нет.
ИЗМЕРЕНИЕ ОПРОВЕРГЛО:
- Лог "vdd on, 3000000 uV" — сенсор УЖЕ получает ровно 3.000В (рельса
  vreg_l11c_3p3 имеет min=3.0В; наш set_voltage был бы no-op).
- Рельса disabled/num_users=0 без модуля ⇒ ВЫДЕЛЕНА сенсору (дисплей её не
  держит, хотя DTS-метка "vdd/vci display"); наш enable даёт ЧИСТЫЙ POR
  (disabled→enabled), а не refcount++.
- Полярность (ACTIVE_LOW→физ. уровни), ширина импульса reset (3мс), пост-reset
  settle (наш 10мс ≥ сток 3мс) — всё совпадает.
⇒ power/reset у нас = сток по ВСЕМ пунктам. НЕ причина гейта.

ИТОГ стены 1: систематически исключены config 1004, калибр-блоб, frameNum,
пропущенный листенер, power/reset/напряжение/POR. Остаток — ВНУТРИ TZ: отработал
ли milan_update_dac_value@0x93fb4 на пути 1005, и удержал ли сенсор DAC-состояние
до чтения гейтом (сон [[spinel-gf-kristall-spit]]). OTP читается ([[spinel-gf-otp-ispraven]]).
Следующий безопасный шаг: трейс — ВЫЗЫВАЕТСЯ ли milan_update_dac_value на
1005/INIT_FINISHED (не срезан ли веткой).

## Стена 1: трейс упёрся в косвенную диспетчеризацию + замер IRQ (3 сент)

- Трейс "вызывается ли milan_update_dac_value на 1005": ТУПИК для статики.
  chip-функции (gf_hw_chip_config_init@0x8c690, milan_init_default_dac@0x8c9a4)
  зовутся через таблицу указателей (0 прямых bl) — путь 1005 статикой не
  проследить. Обработчик 1005 = gf_ta_init_finished@0x16a3c.
- Замер IRQ во время MT_BASE(1110): 0 фронтов, линия покоя=0 весь скан (IRQ
  зарегистрирован, irq 303). Склоняет к "сенсор НЕ захватывал кадры" (гейт упал
  до захвата), но не железно (фоновый FDT-скан мог бы читать по SPI без IRQ).

ground truth нужен = увидеть сам кадр. Путь: заводская raw-capture команда с
правильными параметрами (формат в HAL libgf_hal.so). Агент извлекает формат,
чтобы снять сырой кадр: насыщен (railed фронтенд) ИЛИ гейт валит до захвата.

## ПЕРЕОЦЕНКА стены 1: гейт проходит, кадр ПУСТОЙ (3 сент, ground truth 1098)

Разведчик дал raw-capture рецепт: 1098 TEST_POLLING_IMAGE синхронна, кадр в тело
(row*col int16), за DAC-гейтом. Эксперимент:
- 1098 → rc=0, result=0 ⇒ DAC-ГЕЙТ ПРОШЁЛ (не 1107). Гипотеза "гейт блокирует" —
  НЕВЕРНА. DAC валидны.
- Тело (объявлено 0x4FFA8): только 52-байт заголовок (0x34, 0x40×10 — та же
  структура, что у GET_DEV_INFO), пиксельная область ВСЯ НУЛИ (скан 384КБ, nz>8
  лишь в блоке 0).
⇒ фронтенд НЕ railed/насыщен — сенсор отдаёт ПУСТОЙ (нулевой) кадр.

Согласуется с [[spinel-gf-kristall-spit]] (чтение спящего = нули) и
[[spinel-gf-okno-v-kristall]] (съёмка при IRQ 0x8080). Гипотеза: захват не доходит
до экспозиции — сенсор спит/не экспонирует. Стена 1 = НЕ DAC-гейт, а ПУСТОЙ кадр.
Пробую: разбудить/взвести (1075 фон, [[spinel-gf-preryvanie-rabotaet]]) перед 1098.

## СИНТЕЗ стены 1: гейт проходит, но readout пуст — упор в «запись не доходит» (3 сент)

Тест с пальцем: 1098 С ПАЛЬЦЕМ = тот же пустой кадр (result=0, пиксели нули,
заголовок 0x34/0x40). Палец НЕ изменил ничего.

Связная картина (все факты сходятся):
- DAC-гейт ПРОХОДИТ (1098 result=0) — сенсор при POR сам грузит OTP в DAC,
  reads совпадают с кэшем.
- Но imaging-readout отдаёт НУЛИ и через 1098, и через 1059 (register read) —
  чтение образных данных пусто, палец не меняет.
- OTP/конфиг ЧИТАЮТСЯ ([[spinel-gf-otp-ispraven]]), а вот ЗАПИСЬ в сенсор НЕ
  доходит ([[spinel-gf-zapis-ne-dohodit]]: "чтение работает, запись нет; set_mode
  не программирует кристалл").

ВЫВОД-кандидат: корень — SPI-ЗАПИСЬ в сенсор не доходит. Тогда трастлет не может
перевести сенсор в РЕЖИМ СЪЁМКИ (mode-set = запись регистров) → сенсор не
экспонирует → пустой кадр → count=0 → база не строится. DAC-гейт проходит, т.к.
он про POR-загруженные DAC (reads), а не про imaging-mode (writes).

Это переносит фокус с DAC-гейта на secspi-ЗАПИСЬ. Reads работают, writes нет —
классический признак: направление/CS/такт записи, либо TZ пишет иначе. Следующее:
почему secspi-запись в сенсор не доходит (был бы корнем и «пустого кадра», и
«set_mode не программирует», и невозможности стартовать съёмку).

## КОРРЕКЦИЯ-2: мы читали ЗАГОЛОВОК, не кадр (3 сент, опрос команд)

Опрос 1057/1084/1081/1080/1025 (+ ранее 1098/1089): ВСЕ res=0, и ВСЕ возвращают
ОДИН фиксированный заголовок в telo:
  [+0]=itog(0) [+4]=2 [+8]=0x34(52) [+12]=0|0x42e(1070) [+16..52]=10×0x40(64) далее 0
Десять 0x40 идентичны для любой команды ⇒ это ОБЩИЙ заголовок ответа, НЕ данные.

⇒ Вывод «пустой/placeholder/unbiased кадр» — АРТЕФАКТ. Мы читали заголовок, а не
кадр. Реальные payload'ы (кадр, дампы, OTP, вердикт) идут в ОТДЕЛЬНЫЙ выходной
буфер, который proba НЕ предоставляет/не читает. Настоящий пробел — в оснастке.

Также: [+12]=1070 у 1084/1081/1080/1025 (нужны параметры/предзахват), у 1057 =0.

СЛЕД: разобрать, как трастлет отдаёт payload — механизм выходного буфера
(QSEECom resp-буфер / указатель в запросе). Референс gxfp5130: dumped-данные у
вендора идут отдельным goodix-фреймом; у нас — надо найти, куда трастлет пишет
DUMP_ORIGIN_DATA (gf_ta_dump_origin_data@0x1b6e4) и предоставить этот буфер.
Пересмотреть прежние выводы стены 1 (кадр никогда не наблюдался).

## ПЕРЕЛОМ: критерий — enroll, НЕ кадр (3 сент, палец на КНОПКЕ)

Пользователь: «кадры мы и не должны получать». Верно — match-in-TZ: сенсор
снимает, трастлет сличает ВНУТРИ, наружу только результат+токен. Сырой кадр не
покидает TZ (proba это уже фиксировала, строка 822). Вся погоня за кадром =
не тот критерий. Критерий успеха = enroll строит шаблон, auth сличает.

Прогон enroll (zapis=1) с пальцем НА КНОПКЕ ПИТАНИЯ (раньше тест был на ЭКРАНЕ!):
- ДЕТЕКТ РАБОТАЕТ: одно событие state=2 (=gxfp5130 0x0002 finger-DOWN) itog=0.
- НО: всего 1 касание за 48с, enroll НЕ прогрессирует (state=2, не 4/5), шаблон
  не записан (демон не пишет finger_*).

Диагноз: детект жив, но (а) арминг не держится/не переармится в enroll-цикле
(как в nav: сенсор уходит в 1062 после арма — надо переармить), и/или (б) FDT-база
детекта не построена → детект флаки. FDT-база СТРОИТСЯ ВНУТРИ TZ (нам её видеть
не надо) — но она нужна для НАДЁЖНОСТИ детекта, не для «увидеть кадр».

Прежние выводы стены 1 («пустой/unbiased кадр», «сенсор мёртв») — СНЯТЫ: сенсор
детектит палец. Настоящая задача: надёжный детект + прогресс enroll (переарм,
обработка касания через 1016 до состояний 4/5, сбор фрагментов, 1008/1012).
Референс gxfp5130: enroll = поток кадров под пальцем, K фрагментов по качеству.

## ПРОРЫВ: IRQ-тракт работает, записи доходят (3 сент, TEST_INTERRUPT_PIN)

GF_CMD_TEST_INTERRUPT_PIN(1052): res=0 (пройдено), IRQ-счётчик 0→8, линия дёрнулась,
Linux поймал 8 прерываний. TEST_RESET_PIN1(1050) res=0; 1051 result=1068.

⇒ ДОКАЗАНО:
- Сенсор МОЖЕТ гнать линию прерывания; наш Linux-IRQ (пин 23, RISING) ЛОВИТ её.
- host_irq_set (запись регистров сенсора) ДОХОДИТ и берёт эффект.
- Весь IRQ-тракт исправен: pinctrl/полярность/триггер/обвязка/маршрутизация в Apps.
- Записи в сенсор ДОХОДЯТ. [[spinel-gf-zapis-ne-dohodit]] — артефакт read-back
  (1059-read врёт нулями на спящем сенсоре, но внутренний secspi работает).

ИСКЛЮЧЕНО ОКОНЧАТЕЛЬНО: writes-don't-reach, мёртвая обвязка, неверный pinctrl/
триггер, secure-routing. Всё это опровергнуто — тракт работает.

ОСТАЁТСЯ УЗКО: касание не запускает IRQ, хотя детект армится (1092) и IRQ-выход
исправен (1052). ⇒ сенсор НЕ ДЕТЕКТИТ палец: нет валидной FDT-базы (эталона
детекта). Отклонение ёмкости под пальцем не распознаётся → нет finger-down. Это
внутри-TZ построение FDT-базы (её видеть не надо, но она должна СТРОИТЬСЯ). Ранее
MT_BASE/1005 базу не сохраняли ("check fdt base validation failed"). Следующее:
почему FDT-база не строится/невалидна, хотя DAC-гейт проходит.

## FDT-база детекта: механизм ясен, вход не строит базу (3 сент, агент+тест)

Агент FDT дал механизм:
- MODE_FF(=3) — низкоэнергетический wake-on-finger. Вход в MODE_FF (milan_set_mode
  @0x8abd8, cmp #3 → bl milan_update_fdt_up_base @0x97ae4) СТРОИТ FDT-up базу
  (эталон no-finger) как side-effect. Сенсор сравнивает живой скан с up-базой,
  дёргает FDT-reverse IRQ на отклонение. host_irq_set (регистры 0x120/0x124/0x128)
  маршрутизирует событие на пин (доказанно работает).
- Захват базы: milan_read_fdt_base шлёт chip-команды через gf_milan_secspi_write_cmd
  @0x222cc: opcode 0xc0 (mode/config) → 0xc1 (fdt cmd) → 0xa1 (trigger scan) →
  чтение reg 0xa2 (24 байта, 12 зон). ПОЛЛИНГ, IRQ не нужен.
- base_get_fdt_base @0x4d5b8: при ошибке чтения ОБНУЛЯЕТ count @0x68a234; при
  успехе memcpy в @0x68a1ea, count++. Валидация @0x4d890: нужно count>1 (≥2) И
  |зона[last]-зона[last-1]|<threshold по всем 12 зонам (2 стабильных no-finger кадра).
- НАША ошибка: MT_BASE(1110) строит IMAGE mt-base (не FDT); INIT_FINISHED(1005)
  только ОБНУЛЯЕТ слоты FDT. FDT-up база строится ТОЛЬКО входом в MODE_FF.

ТЕСТ: 1059 set_mode=3 (без пальца) → res=0, но finger-IRQ=0 за 40с касаний.
⇒ вход через 1059-set_mode НЕ построил/не включил базу. Гипотезы: (а) 1059
test-set_mode не идёт через полный milan_set_mode (нет side-effect базы), (б)
значение 3 у 1059 переводится таблицей в иной режим, (в) захват дал <2 кадров
(count<2) — база невалидна, (г) chip-команды 0xc0/0xc1/0xa1 бейлят (сенсор спит
после взведения [[spinel-gf-kristall-spit]] — bus off при захвате).

СЛЕД: точный путь входа в MODE_FF со стороной построения базы (правильная
команда, а не 1059-test-set_mode), и почему захват не даёт ≥2 кадров.

## ⚠️ Перебор режимов ПОДВЕСИЛ планшет (3 сент)

mode_sweep (set_mode 0..8 подряд, 1059) при непрерывном касании — планшет ушёл в
несеть (переиск адреса пуст, ssh timeout). Какое-то значение set_mode (0-8) или
быстрая смена режимов загоняет сенсор/трастлет в состояние, вешающее устройство.
Нужна ФИЗИЧЕСКАЯ перезагрузка (ssh недоступен для sysrq). ОПАСНО: не перебирать
режимы вслепую; set_mode менять по одному с проверкой и паузой.

## Находка: 1059 set_mode — НЕ путь построения базы (3 сент, после ребута)

1059 = строковый диспетчер (gf_ta_test_driver_cmd @0x1d1e0); "set_mode" (стр.
0x344ac6) ветвится на 0x1ee70 → проверка value≤10 → bl 0xa92ec (milan_set_nav_mode
+0x468). Это НЕ gf_hw_set_mode(0x8b604)→milan_set_mode(0x8abd8)→milan_update_fdt_up_
base(0x97ae4). Т.е. 1059 set_mode ставит режим ТОНКИМ test-путём БЕЗ side-effect
построения FDT-up базы. ⇒ set_mode=3 не строил базу — оттого детект не включался.

milan_update_fdt_up_base зовётся ТОЛЬКО из milan_set_mode (case FF=3, @0x8ac74) и
двух других (@0x8b7d8, @0xa3028) — все через function pointer. Значит базу строит
НОРМАЛЬНАЯ операция (nav/auth/enroll через gf_hw_set_mode), а не test-set_mode.

ОСТАЁТСЯ: какая нормальная команда триггерит gf_hw_set_mode(MODE_FF) с построением
базы, и почему enroll(1092)/auth(1061) её не включили (может, ставят IMAGE не FF,
или FDT-скан внутри бейлит). Слепой перебор режимов ОПАСЕН (подвесил планшет).

## Вход в MODE_FF: команды известны, база строится ВНУТРИ TZ (3 сент)

gf_hw_set_mode @0x8b604: w19=режим; w20=код возврата milan_set_mode (через vtable).
- op==1 (@0x8b7bc ldr [x25+108]) → bl 0x963f4 = build FDT-DOWN base.
- w20==0x429(1065) (@0x8b7cc) → bl 0x97ad0 = build FDT-UP base.
Т.е. up-база строится, когда milan_set_mode ВОЗВРАЩАЕТ 1065 ("надо обновить базу").

1059 set_mode → 0xa92ec (тонкий test-путь, БЕЗ базы). Нормальный путь —
gf_hw_set_mode, триггерится операциями auth(1010/1061)/enroll(1092)/nav(1071),
которые МЫ УЖЕ слали. Построение базы — внутреннее, за цепочкой кодов возврата и
chip-команд FDT-скана (0xc0/0xc1/0xa1→reg 0xa2), которые бейлят у нас (count<2).

ВЫВОД: «точная нормальная команда» = auth/enroll/nav (известны, посланы). Стена —
НЕ в команде, а в том, что ВНУТРЕННИЙ FDT-скан не даёт валидный кадр (count<2),
корень внутри TZ (chip-команды/состояние/сон сенсора), из Linux не наблюдаем и
не правится. Механизм картирован полностью; железо+тракт+записи доказанно живы.

## РАЗВОРОТ: нужна РАНТАЙМ-калибровка, не заводской FDT-файл (3 сент, агент хранения)

Хранение = GP TEE Persistent Objects (наш FS-листенер их обслуживает).
gf_fdt_base.so = 24 байта (12 зон × int16) + crc32-обёртка (save_so_content_with_crc
@0x4b964, load @0x4bcb4). count кадров транзиентный в RAM (@0x68a000+564), в файл
не пишется.

Отсутствие gf_fdt_base.so НЕ фатально (gf_base_init_finished @0x4e578: load даёт
rc=1003, идёт дальше). "force update all base" (gf_base_update_finger_base @0x4dd28,
событие==5) РЕАЛЬНО строит все базы через base_update_all @0x4ca84
(milan_update_fdt_up_base, _down_base, nav). Заводской FDT-файл НЕ нужен.

ЦЕПОЧКА к стабильным FDT-кадрам (иначе base_check_fdt_base_validation @0x4d8ec:
count>1 И |зона[i]_n - зона[i]_{n-1}|<threshold по 12 зонам, стрид 0x18; иначе
rc=1003):
1. OTP-read ok → default_dac, tcode∈(0,320], diff∈[300,1700] (gf_init_data_process:
   диапазоны @0x3576d7/0x35777f).
2. check_sensor_dac_validity @0x951d4 проходит (у нас 1098 result=0 — проходит).
3. КАЛИБРОВКА валидна: gf_calibration.so ЛИБО рантайм gf_cali_common_calibration
   @0x5ad5c → base_b/tcode. БЕЗ неё кадры нестабильны → база не строится.

ВЫВОД: наш "прорыв стены 2" неполон — дефолт (Kr=0x2000) даёт НЕСТАБИЛЬНЫЕ кадры.
Нужна НАСТОЯЩАЯ калибровка (base_b/tcode) через рантайм gf_cali_common_calibration
/ BIO_CALIBRATION(1064). ЭТО пропущенный шаг между init и enroll. 1064 с пустым
телом → 1070 (нужны параметры). Файлы: gf_module_info пишется на init;
finger_*/auth_token — после enroll-commit; gf_calibration — после калибровки;
gf_otp_info — ТОЛЬКО в заводском pixel-test (нам не нужен).

## СХОЖДЕНИЕ: пропущенный шаг = 1064 BIO_CALIBRATION (прайминг ЦАП) (3 сент, оба агента)

Стоковый HAL init: gf_hal_reinit @0x19280: open_session → 1000 → reset_chip →
1001 → 1004 → 1005 → hal_milan_init_model → mt_base_load_mt_base_to_ta. ЗАТЕМ
per-chip function_customize (gf_hal_milan_an_series_function_customize @0x5d6bc)
шлёт 1064 BIO_CALIBRATION (@0x5f8f4) + 1066/1049/1058/1082/1102/1103.

1064 → TA gf_cali_calibration @0x58bec / gf_cali_common_calibration @0x5ad5c →
ПИШЕТ банк ЦАП из OTP: milan_common_update_dac_register_from_opt @0xaa3a4 /
milan_update_dac_value @0x93fc8, с проверкой check_sensor_dac_validity @0x95214.
ЭТО прайминг ЦАП = стабильные FDT-кадры (калька gxfp5130 PRIME/DAC).

Каскад после прайминга: первый set-mode → force_update_all_base → build DOWN base
(валиден) → milan_update_fdt_down_base ВОЗВРАЩАЕТ 1065 (mov w19,#0x429 @0x963ec) →
gf_hw_set_mode @0x8b7cc строит UP base (milan_update_fdt_up_base). Альтернатива —
явный MODE_FF=3.

Боевой enroll (не тест): 1006 PRE_ENROLL → 1007 ENROLL (на образец) → 1011/1013
POST_ENROLL. Мы шли тест-путём 1060/1092.

ОТКАЗ У НАС: пропустили 1064 → ЦАП не запраймлен → FDT замороженный → <2 валидных
→ DOWN не строится → 1065 не возвращается → UP-базы нет → нет finger-IRQ.

РЕЦЕПТ: 1005 → 1064(BIO_CALIBRATION) → set-mode/MODE_FF или enroll → базы каскадом.
Нужны точные ПАРАМЕТРЫ тела 1064 (пустое → 1070). Ищу тело у HAL @0x5f8f4.

## 1064 отвергается диспетчером; калибровка-бутстрап = финальная стена (3 сент)

1064 (и 1094/1097) → result(envelope+12)=1070 при ЛЮБОЙ операции в теле+96.
1070 ставится диспетчером test-chip-series ДО запуска ⇒ group-3 test-команды
отвергаются БЕЗ входа в заводской TEST-режим (протокол GRCalibration). 1111
(INIT_MODEL) ломает сессию (-22). init 1000-1005-1014 работает (rc=0).

СИНТЕЗ ПОЛНОГО ПУТИ (оба агента + эмпирика):
- Android-устройство ЗАВОДСКИ откалибровано: gf_calibration.so (base_b/tcode)
  провизится на заводе (GRCalibration), грузится на каждом init. На чистом/
  стёртом устройстве его нет.
- Без калибровки трастлет ставит ДЕФОЛТ (Kr=0x2000) → FDT-кадры НЕСТАБИЛЬНЫ →
  base_check_fdt_base_validation даёт <2 валидных → FDT-база не строится → нет
  detect. DAC-прайминг (milan_update_dac_value из OTP) на 1005 проходит
  (check_sensor_dac_validity ок по 1098), но одного его мало — нужна base_b/tcode.
- Рантайм-калибровка gf_cali_common_calibration @0x5ad5c существует, но
  триггерится либо заводским 1064 BIO_CALIBRATION (нужен TEST-режим/протокол),
  либо косвенно; наш enroll (тест 1060/1092) её не поднял.

ФИНАЛЬНАЯ СТЕНА = калибровка-бутстрап (base_b/tcode). Варианты: (а) войти в
заводской TEST-режим и прогнать 1064 калибровку; (б) сковать gf_calibration.so
с вычисленными base_b/tcode (нужны значения); (в) найти НЕ-тест триггер рантайм-
калибровки в боевом enroll (1006/1007). Всё остальное (железо/тракт/хранение/
путь) — наше и доказано.

## Полная форензика хранения + боевой enroll (3 сент)

DURABLE-разделы проверены (LUN досозданы rescan'ом):
- /persist (sda2, ext4): factory fac_cal ДРУГИХ сенсоров (гиро/аксель/маг/prox/als/
  sar) есть; TEE-объекты (gatekeeper/keymaster/widevine, хешированные имена) есть;
  fingerprint_version (4 байта 7e009578). НО goodix/fdt/калибровки отпечатка НЕТ.
- secdata (sde57, 28.7К): attestation-ELF + Qualcomm-сертификат. Не калибровка.
- storsec (sde55): пусто. logfs: нет goodix.
- Образы прошивки (vendor/odm/etc, super.img): дефолт-калибровки отпечатка НЕТ.

⇒ Фингерпринт-калибровка (gf_calibration.so) была в USERDATA (плейнтекст-путь
/data/vendor_de/0/fpdata/ через FS-листенер), СТЁРТА. Durable источник = OTP в
кристалле GF3658. Калибровка ПЕРЕСОБИРАЕТСЯ рантайм из OTP, нигде не хранится.

Боевой enroll (1006 PRE→1007 ENROLL→1011 POST, группа 1): 1006/1011 itog=0, но
IRQ=0, калибровка/база НЕ построены (демон ничего не писал). Штатный путь тоже не
триггерит рантайм-калибровку у нас.

ФИНАЛЬНАЯ СТЕНА (точно): пустой скан сенсора даёт НЕСТАБИЛЬНЫЕ кадры →
gf_cali_common_calibration не даёт валидный base_b → нет стабильных FDT-кадров →
FDT-база не строится → нет finger-IRQ. Корень: смещён ли фронтенд валидными
OTP-значениями (default_dac/tcode/diff). Триггер рантайм-калибровки: заводской
1064 (test-режим, отвергается 1070) ЛИБО косвенный путь, которого штатный
1006/1007 у нас не поднимает. OTP-значения из Linux не читаются (1059→нули,
TZ-лог закрыт). Всё до калибровки — наше; калибровка-бутстрап за границей TZ.

## ПРОРЫВ: калибровка = группа 6, cmd 1064, тело 96 байт (3 сент)

Перебор группы: 1064 в группе 6/2/5 → result=1003 (params/длина), а НЕ 1070
(отказ диспетчера как в группе 1/3/4). HAL шлёт калибровку ГРУППОЙ 6 (0x22ae4:
orr w0,#6). С объявленной длиной тела 0x60(96) как у HAL → 1064 g6 → result=0
(ПРИНЯТО). Наши прежние вызовы калибровки шли с неверной длиной 0x4FFA8 → 1003.

НО: 1064 g6 96B res=0, и MT_BASE res=0 — калибровка/база НЕ записаны демоном.
Значит команда принята, но не завершает калибровку — нужно ТОЧНОЕ тело
параметров (тип калибровки/кадры), которое строит HAL. op в теле+96 не влияет
(res=0 для всех 0..50) — либо TA сам ставит op, либо поле op у группы 6 иное.

REPLAY-путь: извлечь точное 96-байтное тело калибровки из HAL (invoke group6/
cmd1064 около 0x22ab8) ЛИБО запустить сам HAL. Команда/группа/длина уже наши.

## RUN-THE-HAL: модуль /dev/qseecom ГОТОВ И ДОКАЗАН (3 сент)

Вместо реверса точного 96-байтного тела калибровки — запускаем СТОКОВЫЙ HAL,
он сам гонит калибровку из OTP + enroll, как на Android. Начали с /dev/qseecom.

### Модуль 1: шим /dev/qseecom (linux-rom/fingerprint/qseecom-shim) — ГОТОВ
Тонкий char-dev поверх qcom_scm_qseecom. Транслирует ioctl'ы QSEECom (magic 0x97)
libQSEEComAPI.so в наши SCM-вызовы. Мост app_id: proba(load_keep) грузит goodixfp,
экспортирует spinel_fp_loaded_app_id, шим отдаёт его на LOAD_APP (app_get_id по
имени у нас не работает).
ДОКАЗАНО чистым прогоном: qc_test через /dev/qseecom →
  qseos_version=0x1400000, LOAD_APP 'goodixfp' rc=0 app_id=5.
SEND_MODFD: dma_buf_get+vmap ion-fd → tzmem, патч физ-адреса по cmd_buf_offset,
app_send, копия тела назад. Механика app_send уже доказана proba (res=0 на 1064).
ВАЖНО: load_keep+rmmod proba течёт CMA (образ остаётся у трастлета) → на след.
загрузке alloc -16 EBUSY. proba держать загруженной, не снимать; либо ребут.

### Карта зависимостей (вскрыта)
- libgf_hal.so — ЛОГИКА: простой C-API (gf_hal_function_init, gf_hal_pre_enroll,
  hal_notify_test_bio_hbd_calibration, gf_hal_common_load_otp_info_from_sdcard,
  hal_test_enroll…). НЕ за HIDL/C++-классом. binder НЕ используется (нет
  /dev/binder, ProcessState) — libbinder.so нужен лишь для разрешения символов.
  → dlopen напрямую, без hidl-сервиса.
- libgf_ca.so — ТРАНСПОРТ: gf_ca_invoke_command (послать cmd трастлету через
  QSEEComAPI), gf_sec_ion_* (ion-буферы). Deps: QSEEComAPI, libhardware, cutils,
  ion, c/m/dl, log.
- libQSEEComAPI.so — выделяет общий буфер через ion_open/ion_alloc_fd/ion_close
  (libion.so). Открывает /dev/qseecom (наш шим).
- libion.so (сток) — ТОЛЬКО /dev/ion, отката на dma_heap НЕТ.

### Остаётся (модули run-the-HAL)
2. Шим libion→dma-heap (bionic .so, 3 функции: ion_open→/dev/dma_heap/system,
   ion_alloc_fd→DMA_HEAP_IOCTL_ALLOC, ion_close). Подменяет стоковый libion.
3. Окружение bionic: linker64 + libc/m/dl/c++/cutils/utils/binder/log/hardware из
   стока (source_artifacts/roms/zui-16.0.474/raspakovka-polnaya + super.img).
4. Хранилище: наш spinel_qsee_listener + демон (есть).
5. proba load_keep: грузит goodixfp, питает сенсор, экспортирует id (есть).
6. Харнесс: bionic-исполняемый, dlopen libgf_hal.so → gf_hal_function_init() →
   калибровка из OTP → pre_enroll/enroll.
   Заминка: gf_hal_power_reset идёт через /dev/goodix_fp (нет у нас); питанием
   владеет proba. Возможно нужен stub /dev/goodix_fp или держать питание proba.

## RUN-THE-HAL SMOKE-ТЕСТ ПРОЙДЕН: gf_ca_open_session -> 0 (3 сент, 22:33)

Модули 2-6 собраны и связаны. Bionic-харнесс (свой _start→__libc_init, NDK нет)
dlopen'ит стоковый libgf_ca.so под нашим linker64 и зовёт gf_ca_open_session.

Окружение (всё из одного билда __old__/.camx-work/stage + fp-либы устройства):
  linker64, bionic(libc/m/dl), libc++/cutils/utils/base/log/hardware/binder/
  vndksupport, libQSEEComAPI, НАШ libion(→/dev/dma_heap/system), libgf_ca/hal.
  Харнесс: linux-rom/fingerprint/hal-harness/ (harness.c, start.S, build.sh).

Ключ к успеху: шим /dev/qseecom, query_mode=0 — APP_LOADED_QUERY отвечает «НЕ
загружено», QSEEComAPI идёт штатным путём LOAD_APP, где шим отдаёт реальный id
proba (goodixfp id=5). Прошивка положена в /firmware/image/goodixfp.* (симлинк
на /lib/firmware) — QSEEComAPI читает её ДО ioctl LOAD_APP.

Лог прогона:
  QUERY goodixfp -> НЕ загружено -> LOAD_APP -> id=5 rc=0
  SET_MEM_PARAM sb len=4224            (общий буфер команд зарегистрирован)
  QUERY keymaster64 -> LOAD_APP        (gf_ca_tee_auth пошёл за токеном)
  gf_ca_open_session -> 0              УСПЕХ

ЗНАЕМ ДАЛЬШЕ:
- keymaster64 мост неверен: шим отдаёт ЛЮБОМУ приложению id proba (=5=goodixfp).
  tee_auth вернул 0 (не фатально сейчас), но для enroll нужен реальный keymaster64
  ([[spinel-gf-vhod-keymaster]]). Шим правит: маппить по имени, goodixfp->proba id,
  прочим — честный not-found либо своя загрузка.
- gf_sec_ion_init/close возвращают 1 — некритично (повторный init).

СЛЕДУЮЩИЙ МОДУЛЬ: гнать реальную калибровку из OTP — dlopen libgf_hal.so →
gf_hal_function_init / hal_notify_test_bio_hbd_calibration. Тогда HAL сам построит
base_b/tcode из OTP, чего мы не могли сделать вручную (стена 1).

## RUN-THE-HAL: gf_hal_open ПРОГНАН ЦЕЛИКОМ, упор в шифрованный канал (3 сент 22:45)

Модуль 7 — заглушка /dev/goodix_fp (linux-rom/fingerprint/goodixfp-stub): ioctl'ы
драйвера goodix (магия 'g': RESET=0x6702, SPI_CLK_EN=0x40046705, POWER=0x6707…)
= no-op→0, железом владеет proba. /dev/uinput на планшете есть.

gf_hal_open прогнан ЦЕЛИКОМ через наши шимы. Порядок (все стадии отработали):
  device_open → gf_ca_open_session(0) → enable_power → get_fw_info → reset_chip →
  control_spi_clock → gf_ca_invoke_command → … → function_init → teardown.
ion-шим доказан: alloc 7.3МБ/8КБ/1.5МБ — все rc=0.

УПОР: gf_hal_open -> 1049 (GF sensor broken). gf_ca_invoke_command зовётся 4x в
retry (reset+spi_clk), но НИ ОДНОГО app_send — invoke падает ДО отправки.

КОРЕНЬ: helper gf_ca_invoke_command @0x288c шифрует команду whitebox-крипто
(whitebox_exec ×8, коды ошибок 4099/16386/20482/12290) ПЕРЕД
QSEECom_send_modified_cmd (SEND_MODFD). whitebox_exec И get_key_from_keymaster
ОПРЕДЕЛЕНЫ в libgf_ca (не импорт). whitebox падает ВО ВСЕХ invoke (даже
gf_ca_tee_auth в open_session — оттого app_send не летит и там); open_session
терпит (auth необязателен), chip-init — фатально.

Стена = ШИФРОВАННЫЙ КАНАЛ HAL↔трастлет (whitebox + сессионный ключ). Гипотеза:
сессионный ключ ставит tee_auth через keymaster64, а наш keymaster64 фальшивый
(шим отдаёт ему id=5=goodixfp). Круг: invoke нужен ключ ← ставит tee_auth ←
которому нужен рабочий invoke/keymaster.

СЛЕДУЮЩИЙ МОДУЛЬ: настоящий keymaster64 ([[spinel-gf-vhod-keymaster]]) — грузить
трастлет через app_start, шим маршрутизирует 'keymaster64'->его id, 'goodixfp'->
proba id. ЛИБО разобрать whitebox_exec: откуда берёт ключ/таблицы, нужен ли файл
(листенер) или бутстрап-команда.

## РЕАЛЬНЫЙ БАГ НАЙДЕН И ПОЧИНЕН: пул tzmem; трастлет ожил (4 сент)

Сквозной лог ioctl шима показал: SEND_MODFD (nr=4) ВЫЗЫВАЕТСЯ, но qc_send молча
падал на `qcom_tzmem_alloc(512K)` для тела modfd — общий пул
(qcom_scm_get_tzmem_pool) не даёт крупных областей. Тело modfd = целый ion-буфер
(ifd[0] fd=10 = 1.5МБ от gf_sec_ion_init).
ПОЧИНКА: свой пул qcom_tzmem_pool_new (initial 8М, ON_DEMAND до 64М),
TZ_BODY_MAX=16М. Теперь app_send rc=0, тело переносится, физ-адрес патчится.

Байты: cmd[0..8]=пропатченный физ тела (0xe6b08000) ✓; cmd[8]=счётчик 1..5;
cmd[12]=тип (2 у первой, 1 у прочих). resp[0]: 1-я → 0x3f2, прочие → 0x426.

Расшифровка кодов (GF_ERROR_BASE=1000): 1010=GF_ERROR_GENERIC,
1062=GF_ERROR_SENSOR_NOT_AVAILABLE (прежнее 1049=GF_ERROR_TA_DEAD — был наш баг).

СОСТОЯНИЕ: трастлет жив, принимает команды, отвечает. НО отдаёт
SENSOR_NOT_AVAILABLE — не достаёт сенсор по SPI, т.к. заглушка /dev/goodix_fp
no-op'ит RESET/SPI_CLK/POWER, а HAL ждёт сенсор в свежесброшенном состоянии.
СЛЕДУЮЩЕЕ: заглушка goodix_fp должна РЕАЛЬНО дёргать reset/spi-clk/power через
железо proba (не no-op). proba этим уже владеет — вынести функции.

## ПРОРЫВ (4 сент): 1062 разрушена — сенсор инициализируется через HAL
КОРЕНЬ 1062: spinel_fp_proba load_keep НИКОГДА не питал сенсор. razbudit зовётся
лишь при stupen>=8 (после раннего return load_keep). Сенсор стоял без питания ->
DETECT_SENSOR -> 1062. Плюс кристалл спит после взведения [[spinel-gf-kristall-spit]],
короткий reset его не будит — нужен ПОЛНЫЙ power-cycle.
ПОЧИНКА: (1) load_keep зовёт razbudit (питание+reset-GPIO+IRQ);
(2) proba экспортит spinel_fp_hw_wake (регулятор off/on + толчок); заглушка
/dev/goodix_fp зовёт его на ENABLE_POWER (HAL дёргает в начале gf_hal_open).
РЕЗУЛЬТАТ (воспроизводимо ×3): HAL проходит всю инициализацию, все result=0:
  g2/109->1010(терпимо), g1/1000 DETECT_SENSOR->0, g1/1001 INIT->0, g1/1005->0,
  g1/1111->0, g1/1109->0, g3/1101->0.
НОВАЯ ПРОБЛЕМА: после init харнесс падает SIGBUS (BUS_ADRALN) в libc,
"Using old property service protocol" — вероятно property-сервис (HAL читает
ro.* свойства, а области свойств у нас нет/битая). Следующее: поднять минимальный
property-set ЛИБО перехватить __system_property_get.

## gf_hal_open -> 0 (13 сент): ПОЛНАЯ ИНИЦИАЛИЗАЦИЯ HAL УДАЛАСЬ
Property-шим (linux-rom/fingerprint/prop-shim/libspinelprop.so, LD_PRELOAD)
гасит __system_property_*/property_* -> убрал "old property service protocol".
С ним + power-fix (razbudit в load_keep, hw_wake на ENABLE_POWER):
  gf_hal_open(notify) -> 0  ПОЛНЫЙ УСПЕХ.
Харнесс сделан небуферизованным (setvbuf _IONBF) — иначе вывод терялся при крахе.
SIGBUS был НЕ в open: он в hal_notify_test_bio_hbd_calibration (факторная
тест-функция), которую харнесс зря звал. Краш = C++ вирт-вызов на g_fingerprint_device
(объект тест-обвязки, не настроен в standalone). Разбор через core+gdb:
  pc=x9=0x..411003 (мусор), x30=libgf_hal+0x2430c (hal_notify_test_spi_performance),
  x8=&g_fingerprint_device.
СЛЕДУЮЩЕЕ: не звать hal_notify_test_* (они для factory-обвязки). Гнать НАСТОЯЩИЙ
enroll: gf_hal_pre_enroll -> enroll. Калибровка, вероятно, уже в gf_hal_open
(последовательность 1000/1001/1005/1111/1109/1101 result=0).

## ENROLL: gatekeeper обойдён (13 сент) — hal_test_enroll cmd 1060
Точная причина отказа обычного enroll: cmd 1007 -> TA errno=1056 GF_ERROR_INVALID_
CHALLENGE (нужен валидный challenge из pre_enroll + hat, подписанный gatekeeper;
ключ через keymaster - send_hmac_key падает 1010, keymaster64.mdt нет).
ОБХОД: hal_test_enroll(0) -> cmd 1060 GF_CMD_TEST_UNTRUSTED_ENROLL, MODE_IMAGE ->
вернул 0 (стартовал БЕЗ gatekeeper). Это путь 1060 из [[spinel-gf-rezhim-zapisi]].
Крах в gf_hal_notify_acquired_info (цепочка device->notify не настроена standalone,
её ставит HIDL-сервис) снят ПЕРЕХВАТОМ notify-функций в prop-шиме (экспортим
gf_hal_notify_acquired_info/enrollment_progress/error_info/... -> печать). Работает:
[NOTIFY] acquired 0x3e9, без краха.
ОТЛАДКА-ОРАКУЛ: prop-шим перехватывает liblog (__android_log_*) -> все логи HAL в
stderr. Netlink пальца: proba unit 29, ioctl GF_IOC_INIT отдаёт unit HAL'у,
device_enable стартует поток, portid ловится, HAL получает события.
ОСТАЁТСЯ: на netlink-событие сенсор отдаёт irq_type=INVALID(0) - валидный
capture-IRQ на касание не рождается (арминг сенсора в MODE_IMAGE / реальное
касание). Следующее: проверить, что касание реально даёт IRQ (лог в proba nl_send),
и арминг сенсора для capture.

## ЗАХВАТ ОТПЕЧАТКА УДАЛСЯ (13 сент, пальцевой заход) 🎯
Реальное касание -> полный конвейер стокового HAL на mainline отработал:
  [gf_hal_open] detect sensor success
  irq_pre_process GF_IRQ_FINGER_DOWN_MASK (0x0002) - палец обнаружен
  irq_pre_process GF_IRQ_IMAGE_MASK - кадр готов
  irq_image_test pre process err=GF_SUCCESS - обработка кадра успешна
  hal_milan_set_config_data update_config_data success
  14x gx_ta_send_modified_cmd_req success, БЕЗ крахов, hal_test_enroll->0.
Сенсор Goodix GF3658 снимает отпечаток под нашим Linux через run-the-HAL.
ИЗВЕСТНО: шквал IRQ (2966 событий ~50/с при удержании пальца) - [[spinel-gf-lavina-preryvanij]];
не уронил, но для нативной реализации нужен edge/маскирование IRQ на время обработки.
ОСТАЁТСЯ к полному enroll/auth: многокадровый enroll (hal_test_enroll = 1 тест-кадр),
untrusted-хранение образца (1060/1061), затем нативная реализация без android-либ.

## B: KEYMASTER-КАНАЛ РАБОТАЕТ (13 сент) — gf_hal_open->0 с hmac
keymaster уже загружен XBL: app_get_id("keymaster64")=id=1. proba экспортит
spinel_km_app_id, шим маршрутизирует keymaster64/gatekeeper->id=1. Симлинки
/firmware/image/keymaster64.*->km41.* (QSEEComAPI читает файл до LOAD_APP).
get_key_from_keymaster УСПЕХ (ключ 152Б). send_hmac_key УСПЕХ -> goodixfp-TA
принял ключ -> detect success -> init finished -> gf_hal_open->0.
БАГ, что мешал (казался SHK-провалом): наш qc_send отдавал tz_resp с ЗАСТОЕМ из
пула (ответ keymaster: offset=12), CA читал как ret=12 -> "TA_DEAD". ПОЧИНКА:
memset(tz_resp,0) перед app_send. SHK-риск НЕ реализовался - ключ принят.
СЛЕДУЮЩЕЕ: боевой enroll = нужен HAT от gatekeeper. gatekeeper тоже km41 (id=1):
GK_ENROLL(пароль) -> GK_VERIFY(challenge из pre_enroll) -> подписанный
hw_auth_token -> gf_hal_enroll(dev,HAT,gid,timeout) -> запись+хранение.
Формат GK в [[spinel-gf-format-gatekeeper]].
