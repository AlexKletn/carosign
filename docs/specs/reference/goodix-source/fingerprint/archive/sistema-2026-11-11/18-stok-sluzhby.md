# 18 — Сток: службы QSEE и Android (`qseecomd`, `libdrmfs`, `librpmb`, listener)

Наш путь заменяет это своими модулями. Здесь — что именно он заменяет, по
строкам и адресам из двоичных ✅BIN и по нашему разбору (RAZBOR.md,
SLUSHATEL-PLAN.md).

## 18.1 `qseecomd` (Android)

* Демон-посредник между Android-библиотеками и ядром QSEE (`/dev/qseecom`).
* Держит **слушателей** и вызывает их ответы по `listener_id`.
* Для файловой службы — связка с `libdrmfs.so`; для защищённой записи —
  `librpmb.so`.
* На планшете слушатели, которые мы видели в стоковом журнале:
  **`0xa`** (FS, наш `listener_id=10`), **`0x2000`** (RPMB), **`0x7000`**
  (GPFS, сток `libdrmfs.so` регистрирует `0x7e000` байт @`0x5a7c`).

## 18.2 `libdrmfs.so` (файловая служба)

| элемент | адрес | что |
|---|---|---|
| размер GPFS-буфера | `0x5a7c` | `0x7e000` |
| команды GPFS | `0x5c3c–0x5c50`, `0x5cd8–0x5cf0` | `gpfs_start`: 0…3 с полным путём, 4…11 с корнями |
| `GET_ERRNO` | `0x6698–0x66b8` (сохранение), `0x482c–0x4834` (выдача) | ошибка хранится отдельно от `ret` |
| FTS | через `cmnlib64.b02` @`0x18afc` | обращается к службе `0x7000` |
| режим хранилища | `goodixfp.flat` @`0x337d3c` = 2 | открытие на запись = `0x242` @`0x4244–0x4258` |

## 18.3 `librpmb.so`

| элемент | адрес | что |
|---|---|---|
| геометрия | `0x746c–0x7478` | `sectors = младшее слово счётчика блоков / 2` |
| чтение | `0x5cd4–0x5d04` | запрос по `+12`, **ответ кладётся с `+0x14`** |
| запись | `0x5d8c`, `0x7abc` | число кадров в группе — поле `+20` |

## 18.4 QTI `rpmb_service.h` (договор TZ ↔ слушатель)

```
0x101 INIT      → num_sectors, rel_wr_count
0x102 READ      → req_buff_offset, num_sectors;   ответ с +0x14
0x103 WRITE     → фаза данных (nsec>0) и фаза результата (nsec=0)
0x104 PARTITION
```

## 18.5 Стоковое ядро `qcom_scm` / qsee

* `qcom_scm_qseecom_call` — общий вызов; при `BLOCKED_ON_LISTENER` /
  `INCOMPLETE` будит слушателя.
* Неизвестный слушатель → статус `0xffffffff` (`0x7e237c–0x7e2388`,
  `0x7e212c–0x7e2154`).
* Занятый слушатель → ждать (`0x7df77c–0x7df844`).
* `CONTINUE` — `0x320000fc`, кэш до и после.
* Регистрация — `0x32000206`, снятие — `0x32000202`.

## 18.6 Си-версия: соответствие

| служба стока | наша замена | где |
|---|---|---|
| `qseecomd` | `spinel_qsee_listener.ko` + `spinel-lstnrd` | `listener/` |
| `libdrmfs` (FS) | `serve()` в `spinel-lstnrd.c` | строки 519+ |
| `libdrmfs` (GPFS) | `serve_gpfs()` | строки 445+ |
| `librpmb` | `serve_rpmb()` + `rpmb_sg()` | строки 254+ |
| драйвер goodix | `spinel_fp_proba.c` (загрузка/питание/IRQ) | `proba/` |
| `libQSEEComAPI` | `spinel_qseecom.c` | `qseecom-shim/` |
