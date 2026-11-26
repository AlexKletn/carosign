# 00 — Как устроено целиком

## Слои, сверху вниз

```
  Android-слой (сток, служит эталоном)          Наш путь (работал на C)
  ──────────────────────────────────────       ─────────────────────────
  libgf_hal.so      оркестровка, режимы,        (перенесено в Rust:
                    команды 1000…1111,           goodix.rs, irq.rs,
                    маска/операция, уведомления   authentication.rs)
  libgf_ca.so       канал к приложению TZ       --
  libQSEEComAPI.so  ioctl /dev/qseecom          spinel_qseecom.c  (шим)
  libion / dma-heap память под тела команд      native_device.rs  (Rust)
  libdrmfs.so       файловая служба FS          spinel-lstnrd.c + demoн
  librpmb.so        защищённая запись RPMB      spinel-lstnrd.c (sg)
                    GPFS 0x7000                 spinel-lstnrd.c
  Android HAL и прописи                         spinel_goodixfp.c (заглушка)
  ядерный драйвер goodix (питание, IRQ)         spinel_fp_proba.c (владелец)
  ядро QSEE (qcom_scm)                          qcom_scm + spinel_qsee_listener.ko
  TZ: goodixfp, keymaster64, cmnlib(64)
```

## Кто кого зовёт

### Прямой путь (наш, Rust) — так работало на C

1. Rust открывает `/dev/qseecom`, делает `LOAD_APP goodixfp` → шим отдаёт id
   из `spinel_fp_loaded_app_id` (его выставила proba при `load_keep`).
2. Rust выделяет DMA (dma-heap), собирает кадр команды (128 байт заголовка +
   тело), пишет тело в DMA, вызывает `SEND_MODFD_CMD` — шим копирует тело в
   tzmem и **вписывает его физический адрес в начало заголовка** ✅SRC.
3. TZ выполняет команду и может вернуть `BLOCKED_ON_LISTENER` / `INCOMPLETE` —
   тогда ядро (наши `qcom_scm` правки) будит `spinel_qsee_listener`, тот
   выставляет запрос демону, демон выполняет файловую операцию (FS/GPFS/RPMB)
   и возвращает ответ, ядро продолжает SCM.
4. Событие пальца приходит **не от TZ**, а от ядра: `spinel_fp_proba` держит
   IRQ-линию датчика и шлёт netlink-событие на unit 29; получатель — тот, кто
   зарегистрировал портид (стоку регистрирует driver ioctl'ом `GF_IOC_INIT`,
   наш Rust — тоже, см. [02](02-yadro-proba.md)).
5. Rust по событию делает `1016` (а не «ждёт следующего»), затем на успех —
   `1086`.

### Эталон стока (HAL через bionic) — работает «в лоб»

`fp_harness` → `auth-probe` → `libgf_hal.so` → `libgf_ca.so` →
`libQSEEComAPI.so` → `/dev/qseecom` (наш шим) — дальше то же, что выше.
Жизнь даёт `auth_ok`, `match_score=35` ✅MSR.

## Что где живёт по файлам

| часть | файл | строк |
|---|---|---|
| шим `/dev/qseecom` | `linux-rom/fingerprint/qseecom-shim/spinel_qseecom.c` | 477 |
| proba: загрузка приложения, питание, IRQ, netlink | `proba/spinel_fp_proba.c` | 2492 |
| proba: счётчик краёв (испытание) | `proba/spinel_fp_irq_count.c` | 145 |
| слушатель TZ: регистрация, буферы, проводник к демону | `listener/spinel_qsee_listener.c` | 389 |
| слушатель TZ: состояние и переходы | `listener/listener-state.h` | 43 |
| демон: FS, GPFS, RPMB | `listener/spinel-lstnrd.c` | 768 |
| заглушка `/dev/goodix_fp` | `goodixfp-stub/spinel_goodixfp.c` | 132 |
| Rust: договор с приложением | `userspace/rust/spinel-fingerprint/src/goodix.rs` | 1204 |
| Rust: события и череда 1016 | `…/src/irq.rs` | 296 |
| Rust: машина распознавания | `…/src/authentication.rs` | 327 |
| Rust: Linux-переходники | `…/src/native_device.rs` | 736 |

## Правило, которое вывелось из замеров ✅MSR

**Каждая часть нашего пути уже проверена в одиночку; не сходится их
соединение.** Поэтому спецификация ниже — не «как задумано», а «что делать
обязательно»: порядок вызовов, какие ioctl, где чьи буферы, кто кого будит.
Rust должен повторять именно эти шаги, а не стоковый HAL целиком.
