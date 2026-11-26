# Service contracts: FS, GPFS, RPMB

Our daemon `spinel-lstnrd` serves three TrustZone services. The contracts below
are ✅SRC, cross-checked against the stock binaries ✅BIN.

## 1. FS — where paths go ✅SRC

All TZ paths (`/data/vendor_de/0/fpdata/…`, `/persist/data/…`) are remapped
under `/root/spinel-fp`; intermediate directories are created by `mkparents()`.

### Command codes

```
0x202 OPEN      0x203 OPENAT   0x204 UNLINKAT  0x205 FCNTL    0x206 CREAT
0x207 READ      0x208 WRITE    0x209 CLOSE     0x20a LSEEK    0x20b LINK
0x20c UNLINK    0x20d RMDIR    0x20e FSTAT     0x20f LSTAT    0x210 MKDIR
0x211 TESTDIR   0x212 TELLDIR  0x213 REMOVE    0x214 CHOWN_CHMOD
0x215 UNUSED    0x216 SYNC     0x217 RENAME    0x218 PAR_FR_SIZE
0x219 DIR_OPEN  0x21a DIR_READ 0x21b DIR_CLOSE 0x21c GET_ERRNO
```

### Layouts (packed, offsets in bytes)

| command | request | reply |
|---|---|---|
| OPEN | `cmd@0, path[256]@4, flags(int)@260` | `cmd@0, ret(int)@4` |
| CREAT | `cmd, path[256], mode(u32)@260` | `cmd, ret` |
| READ | `cmd, fd(int)@4, count(u32)@8` | `cmd, buf[20000]@4, ret(i32)@20004` |
| WRITE | `cmd, fd, buf[20000]@8, count@20008` | `cmd, ret` |
| CLOSE / FSTAT | `cmd, fd` | `cmd, ret` (FSTAT adds `tz_stat` at `@4`) |
| LSEEK | `cmd, fd, offset(i32), whence(int)` | `cmd, ret` |
| MKDIR | `cmd, path[256], mode` | `cmd, ret` |
| RENAME | `cmd, old[256]@4, new[256]@260` | `cmd, ret` |
| GET_ERRNO | — | `cmd, errno` |

`tz_stat` copies the `struct stat` fields into its own set (dev, mode, nlink,
uid, gid, size, blksize, blocks, ino).

### Mandatory details ✅SRC

1. **The error is remembered separately.** `fs_error_number` is a global, and
   `GET_ERRNO` (0x21c) returns it. The stock (`libdrmfs.so` @`0x6698–0x66b8`,
   @`0x482c–0x4834`) returns exactly that stored value, not the current `errno`.
   ✅MSR This is what made template writes fail with `1035`.
2. `MKDIR` on `EEXIST` → `0`.
3. `TESTDIR` on a non-directory → `ret=-1`, `errno=ENOTDIR`.
4. The read buffer is 20 000 bytes (`DATA_LEN`); larger requests are truncated.
5. `CREAT` opens `O_CREAT|O_WRONLY|O_TRUNC`, mode from the request `& 0777`.
6. An unknown command is a refusal, not an empty success.

## 2. GPFS — buffer and framing

* Size — **`0x7e000`** (504 KiB); the stock `libdrmfs.so` registers that much
  ✅BIN (@`0x5a7c`).
* Request frame: `cmd(u32)@0`, `path[256]@4`, second path field `@0x104`,
  offset `i32@0x104`, length `u32@0x108`, backup flag `u32@0x10c`, data from
  `@0x110` (for writes).
* The reply goes into the same buffer: `cmd@0`, `error(int)@4`, and for reads
  and writes also `count(u32)@8`; read data from `@12`.

### Command numbers — the generation matters ✅BIN

```
cmd = group * 4 + op,  op: 0 read, 1 write, 2 delete, 3 rename
```

* `libdrmfs.so` `@0x5c3c–0x5c50`, @`0x5cd8–0x5cf0`: this tablet's stock sends
  **commands 0…3** (full path, group 0) — the Nord4 port expected 4…11 and
  rejected a real request.
* `cmd == 12` — version: reply `version=2@+4`, `ok=0@+8`.

### Roots per group

| group | prefix |
|---|---|
| 0 | `""` (full path) |
| 1 | `data/vendor/tzstorage/` |
| 2 | `mnt/vendor/persist/data/` |

The name is validated: empty, of length `NAME_LEN`, or containing `..` in any
segment → `EINVAL`.

### Write behaviour ✅SRC

1. `mkparents(path)` — creates the directories.
2. When `backup != 0` — `gpfs_backup()`: `path.bak` receives the current
   contents, `fsync`, errors are not swallowed.
3. `pwrite(fd, buf + 0x110, count, offset)`, `O_CREAT|O_RDWR|O_SYNC|O_CLOEXEC`,
   mode `0600`.
4. Limits: `0 ≤ offset`, `count ≤ 500 KiB`, `count ≤ bufsize − 0x110`.
5. Any error: `error` and `count=0` in the reply, and no data is written.

## 3. RPMB — messages (packed)

```
0x101 INIT      req: cmd, version            res: cmd, version, status,
                                                num_sectors, rel_wr_count
0x102 READ      req: cmd, num_sectors, req_buff_len, req_buff_offset,
                     version, rel_wr_count
                res: cmd, status, res_buff_len, res_buff_offset, version
0x103 WRITE     same req/res as READ
0x104 PARTITION req: cmd, version, dev_id   res: cmd, status,
                                                num_partitions, rsp_buff_offset
```

⚠️ `req_buff_len` in a READ is the **address of the reply length**, not the
length of the request (`librpmb.so` @`0x5cd4–0x5d04`).

## Requirements for Rust

* FS: the `cmd_id` contract is 1:1, with no "smart" additions.
* FS: the same two roots, remapping as an explicit rule, not a per-handler
  prefix hack.
* FS: `GET_ERRNO` lives separately from `ret`.
* GPFS: support **both** number generations — 0…3 (full path, our tablet) and
  4…11 (with roots).
* GPFS: backup by the `@0x10c` flag; `fsync` is mandatory; `O_SYNC`, mode
  `0600` — the file is a protected template, not an ordinary write.
