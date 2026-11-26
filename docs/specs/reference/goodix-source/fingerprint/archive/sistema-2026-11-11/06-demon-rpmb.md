# 06 — Демон: RPMB (слушатель `0x2000`)

Часть `spinel-lstnrd.c` ✅SRC. Протокол — QTI `rpmb_service.h` (Android), поверх
UFS. Нужен TZ для защищённой записи (Gatekeeper, шаблоны).

## Сообщения (packed)

```
0x101 INIT     req: cmd, version            res: cmd, version, status,
                                                num_sectors, rel_wr_count
0x102 READ     req: cmd, num_sectors, req_buff_len, req_buff_offset,
                    version, rel_wr_count
               res: cmd, status, res_buff_len, res_buff_offset, version
0x103 WRITE    тот же req/res, что READ
0x104 PARTITION req: cmd, version, dev_id   res: cmd, status,
                                                num_partitions, rsp_buff_offset
```

## Геометрия ✅BIN (librpmb.so @`0x746c`)

Из sysfs SCSI-устройства:
`unit_descriptor/logical_block_count` (блоки 256 Б),
`logical_block_size` (должен быть `8`, т.е. 256), `geometry_descriptor/rpmb_rw_size`.
`sectors = blocks / 2`, `rel_wr = rpmb_rw_size`.

✅MSR На планшете: `0x10000` блоков, `rpmb_rw_size=0x40` → **32768 секторов**,
64 кадра на запись. Старое значение `0x1000` было ошибкой.

## Устройство — только по адресу ✅SRC

Номер `sg` гуляет при перечислении UFS. Ищем в `/sys/class/scsi_generic` звено,
адрес которого кончается на **`:49476`** (`0xc144` — SCSI well-known RPMB LUN).
На чужих устройствах RPMB-команды не посылать.

## Кадр — через SG_IO

* CDB(12): `op, 0xEC, 0x00, 0x01, 0,0, len[31..0] BE, 0,0`; `op` `0xB5` —
  OUT (`SECURITY PROTOCOL OUT`), `0xA2` — IN.
* Время — 5000 мс; `UNIT ATTENTION` (ключ `6`, `ASC 0x29`) повторяется до
  4 раз — как в стоковом `qseecomd`.
* ⚠️ `SG_IO` **не умеет** работать с `mmap`-нутым tzmem (`VM_PFNMAP`), поэтому
  данные ходят через `g_bounce` (64 кадра).

## Чтение (0x102) ✅BIN (librpmb.so @`0x5cd4–0x5d04`)

1. Кадр-запрос лежит по `req_buff_offset`; `req_buff_len` — **адрес длины
   ответа**, а не длина запроса.
2. OUT кадра-запроса (`0xB5`), затем IN `num_sectors*512` (`0xA2`).
3. Данные кладутся **по `sizeof(res)`** (то есть `@0x14`), `res_buff_offset` =
   `sizeof(res)`, `res_buff_len` = прочитано.

## Запись (0x103) ✅SRC/✅BIN

QSEE дробит запись на **две фазы**:

* `num_sectors > 0` — фаза данных: OUT кадров по группам по `rel_wr_count`,
  каждая группа заканчивается result-request (кадр с `[511]=0x05`), их ответ
  отбрасывается, пока не последняя группа.
* `num_sectors == 0` — фаза результата: OUT `0x0005` + IN кадра результата.
  ⚠️ Сделать всё за один вызов нельзя — вторая фаза прочтёт пустоту.
* `num_sectors == 0` в первой фазе — **чистый no-op** (столько же, сколько в
  `rpmb_ufs_write`, где `udiv(0,50)=0`).
* Проверки: `num_sectors % rel_wr_count == 0`, смещение не выходит за буфер.

## Требования к Rust

* Геометрия — чтением sysfs, а не константой.
* Устройство — по `:49476`, не по номеру `sg`.
* Две фазы записи как выше; никаких «write одним разом».
* MAC ответа проверять (C-версия его не проверяла — так записано в аудите
  как ограничение).
