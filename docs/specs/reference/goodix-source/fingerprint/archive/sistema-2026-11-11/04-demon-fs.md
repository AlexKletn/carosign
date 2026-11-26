# 04 — Демон: файловая служба FS (слушатель `0xa`)

Файл: `linux-rom/fingerprint/listener/spinel-lstnrd.c` ✅SRC.
Протокол — `minkipc libfsservice/fs_msg.h` (Android). Все структуры
`#pragma pack(push, 1)`, первое слово — `cmd_id`.

## Куда ходят пути

✅SRC Все пути TZ (`/data/vendor_de/0/fpdata/…`, `/persist/data/…`) ремапятся под
`/root/spinel-fp`; промежуточные каталоги создаются `mkparents()`.

## Коды команд

```
0x202 OPEN      0x203 OPENAT   0x204 UNLINKAT  0x205 FCNTL    0x206 CREAT
0x207 READ      0x208 WRITE    0x209 CLOSE     0x20a LSEEK    0x20b LINK
0x20c UNLINK    0x20d RMDIR    0x20e FSTAT     0x20f LSTAT    0x210 MKDIR
0x211 TESTDIR   0x212 TELLDIR  0x213 REMOVE    0x214 CHOWN_CHMOD
0x215 UNUSED    0x216 SYNC     0x217 RENAME    0x218 PAR_FR_SIZE
0x219 DIR_OPEN  0x21a DIR_READ 0x21b DIR_CLOSE 0x21c GET_ERRNO
```

## Раскладки (packed, смещения в байтах)

| команда | запрос | ответ |
|---|---|---|
| OPEN | `cmd@0, path[256]@4, flags(int)@260` | `cmd@0, ret(int)@4` |
| CREAT | `cmd, path[256], mode(u32)@260` | `cmd, ret` |
| READ | `cmd, fd(int)@4, count(u32)@8` | `cmd, buf[20000]@4, ret(i32)@20004` |
| WRITE | `cmd, fd, buf[20000]@8, count@20008` | `cmd, ret` |
| CLOSE/FSTAT | `cmd, fd` | `cmd, ret` (у FSTAT — `tz_stat` по `@4`) |
| LSEEK | `cmd, fd, offset(i32), whence(int)` | `cmd, ret` |
| MKDIR | `cmd, path[256], mode` | `cmd, ret` |
| RENAME | `cmd, old[256]@4, new[256]@260` | `cmd, ret` |
| GET_ERRNO | — | `cmd, errno` |

`tz_stat` копирует поля `struct stat` в свой набор (dev, mode, nlink, uid,
gid, size, blksize, blocks, ino).

## Обязательные подробности ✅SRC

1. **Ошибка запоминается отдельно.** `fs_error_number` — глобальная, её
   возвращает `GET_ERRNO` (0x21c). Сток (`libdrmfs.so` @`0x6698–0x66b8`,
   @`0x482c–0x4834`) отдаёт именно сохранённое значение, а не текущий `errno`.
   ✅MSR Из-за этого запись шаблона падала на `1035`.
2. `MKDIR` при `EEXIST` → `0`.
3. `TESTDIR` не каталог → `ret=-1`, `errno=ENOTDIR`.
4. Буфер чтения — 20000 байт (`DATA_LEN`), бОльшие запросы режутся.
5. `CREAT` открывает `O_CREAT|O_WRONLY|O_TRUNC`, режим из запроса `& 0777`.
6. Неизвестная команда — отказ, а не «пустой успех».

## Требования к Rust

* Договор `cmd_id` — 1:1, без «умных» дополнений.
* Пути — те же два корня, ремапинг явным правилом, а не подстановкой префикса
  в каждом обработчике.
* `GET_ERRNO` живёт отдельно от `ret`.
