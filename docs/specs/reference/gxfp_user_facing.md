# Пользовательская часть gxfp5130 — описание

> Охват: всё, что стоит **над** драйвером устройства — логин-стек, демон аутентификации,
> клиенты, PAM-модуль, оверлей-плац, GUI, хранилище шаблонов/секретов, пакетирование,
> сервисные юниты и операционные утилиты.
> Сознательно **не** описаны: wire-протокол сенсора, профиль ChicagoHS, шаги активации/калибровки,
> захват кадров (`gxfp-driver`, `gxfp-stream`), математика сопоставления (NCC/фазы/AKAZE)
> и обучение эмбеддера — только их роль как «чёрных ящиков», на которые опирается
> пользовательский слой.

---

## 1. Общая картина

```
            login / sudo / polkit / lock-screen / greeter
                              │  стек PAM
                              ▼
                    pam_gxfp.so ── unix-сокет ──► gxfp-authd (root)
              (непривилегированный клиент)        │ ├─► /dev/gxfp
                    gxfp-authctl ─────────────────┘ ├─► templates.redb (AES-256-GCM)
                                                      ├─► key (0600, root)
                                                      ├─► secrets/*.cred (systemd-creds + TPM)
                                                      └─► абстрактный dgram-сокет
                                                              ▼
                                                        gxfp-plaque (user)
                                                          оверлей Wayland
```

Ещё три независимых поверхности, не участвующих в логине:

| Поверхность | Что делает | Где живёт |
|---|---|---|
| `gxfp-gui` + `gxfp-gui-backend` | визуальная отладка/демонстрация enroll/verify (egui) | `crates/apps/gxfp-gui*` |
| `gxfp-cli` | инструменты сенсора, офлайн enroll/verify, PSK | `crates/apps/gxfp-cli` |
| `gxfp-monitor` | пассивный сэмплер нагрузки демона | `packaging/gxfp-monitor.sh` |

Ключевой принцип: **демон — единственный компонент с сенсором и ключами.** Клиенты тонкие,
не трогают железо и не читают шаблоны напрямую (`docs/FINGERPRINT_LOGIN.md`).

---

## 2. `gxfp-authd` — демон аутентификации (root)

**Путь:** `crates/apps/gxfp-authd/` · бинарь `gxfp-authd` · unit `packaging/gxfp-authd.service`

### 2.1 Что он владеет

Все ресурсы открываются один раз при старте (`serve.rs::serve`):

1. **PSK** (`--psk-raw32`, по умолчанию пусто → обязателен; на деле `/var/lib/gxfp/psk_raw32.bin`) —
   читается файлом и передаётся в сессию сенсора.
2. **Сессия сенсора** — `Session::open(dev, psk)` собирает `Dev` + `SystemClock` + `TlsService` +
   `ChicagoHsProfile` + `Driver` (probe → activate → dark reference → prime FDT).
3. **Защищённое хранилище** (`store_setup.rs::SecureStore::open`):
   - `--key` → `/var/lib/gxfp/key`: 32-байтный AES-256-GCM ключ, `0600`, генерация из OS CSPRNG
     при первом запуске (`gxfp-keystore::KeyfileKeyProvider`), в памяти — в `Zeroizing`;
   - `--store` → `/var/lib/gxfp/templates.redb`: redb, хранит **только** шифротекст;
   - каталоги принудительно `0700`.
4. **TPM-хранилище секретов** (`--secrets-dir` → `/var/lib/gxfp/secrets`, `secret_store.rs`):
   на старте проверяет `systemd-creds has-tpm2` и **громко предупреждает**, если запечатать нельзя.
5. **Эмбеддер (опционально)** (`--embed-model`, `--embed-device cpu|gpu|npu`): загружается один раз
   (компиляция модели дорогая); `--verify-matcher embed|hybrid` без модели — ошибка старта,
   а не отказ по запросу.
6. **Unix-сокет** (`--socket` → `/run/gxfp/authd.sock`): каталог создаётся `0755`, сокет — `0666`
   (доступ регулируется по `SO_PEERCRED`, а не правами файла), устаревший сокет удаляется.

Обрабатывает подключения **последовательно** — сенсор физически один.

### 2.2 Конфигурация

Три слоя: **встроенные дефолты < TOML-файл < флаги CLI** (`cli.rs`, `config_file.rs`).

- Файл: `/etc/gxfp/authd.toml` автоматически, если существует; либо `--config <path>`.
- Ключи — ровно имена флагов без `--`; `deny_unknown_fields` → опечатка в ключе = ошибка,
  а не молча проигнорированная настройка.
- Шаблон с комментариями: `packaging/authd.toml.example` (в пакете — `/etc/gxfp/authd.toml`,
  `backup=` в PKGBUILD, т.е. pacman не затрёт правки).

**Структура конфига:** пути (`dev`, `socket`, `store`, `key`, `secrets-dir`, `psk-raw32`,
`embed-model`, `embed-device`) + `Policy`.

Таблица ручек `Policy` (`policy.rs`) с дефолтами:

| Флаг | Дефолт | Смысл (пользовательская сторона) |
|---|---|---|
| `--verify-threshold` | 0.75 | нижняя граница score для «свой палец»; выбран по данным железа в зазоре между genuine 0.81–0.86 и худшим импостором 0.72 |
| `--verify-matcher` | `ncc` | `ncc` \| `phase` \| `embed` \| `hybrid` (в пакете — `hybrid`) |
| `--embed-threshold` | 0.9 (пакет: 0.75) | порог косинуса эмбеддера |
| `--verify-max-rotation` | 25° | толерантность к наклону пальца (NCC) |
| `--verify-attempts` | 3 | сколько раз можно «положить заново» |
| `--verify-finger-timeout` | 300 с | сколько ждать палец, прежде чем ответить «reject»; **0 = вечно** |
| `--enroll-finger-timeout` | 20 с | пауза без повторного прижатия завершает enrollment |
| `--enroll-min-coverage` | 40 | минимум пространственно разных видов; «растирание одной точки» не засчитывается |
| `--enroll-max-passes`, `--enroll-min-new`, `--enroll-dry-passes` | 8 / 2 / 1 | когда считать отпечаток «полным» |
| `--set-max` | 120 | потолок видов в наборе |
| `--set-novelty` | 0.9 | NCC, при котором вид — дубликат последнего |
| `--sweep-frames` | 300 | кадров на проход включает/выкл. |
| `--min-coherence` | 0.3 | порог quality-гейта: ниже — считаем «пальца нет» |
| `--burst`, `--fuse-size` | 3 / 4 | кадров на прижатие и сколько сливать в один кадр |

Ограничение `read_psk_file` в CLI сейчас намеренно ослаблено до 1..=48 байт (диагностика
48-байтного нулевого PSK) — см. `gxfp-cli/src/args.rs`.

### 2.3 Протокол «демон ↔ клиент» (`gxfp-auth-proto`)

Каждое подключение = **один** `AuthRequest`; ответ — поток `AuthResponse` до терминального кадра.
Кадры: `u32` LE длина + bincode (лимит `MAX_MESSAGE_BYTES`).

**Запросы** (`auth_request.rs`):

| Запрос | Поля | Права |
|---|---|---|
| `Enroll` | `user`, `finger`, `swipe` | не-root — только на себя |
| `Verify` | `user`, `with_secret` | не-root — только на себя; секрет выдаётся только root |
| `List` | `user` | не-root — только на себя |
| `Delete` | `user`, `finger` | не-root — только на себя |
| `SetSecret` | `user`, `secret` | **только root**, без «сведения к себе» |
| `ClearSecret` | `user` | **только root** |

**Ответы** (`auth_response.rs`): не термальные `Prompt { text, timeout_secs }`, `Progress(String)`,
`Secret(SecretBytes)`; терминальные `Accepted`, `Rejected`, `Ok`, `Fingers(Vec<String>)`, `Error(String)`.

Детали дизайна, важные для UX:

- `Prompt.timeout_secs` — дедлайн **передаётся клиенту**, чтобы тот мог показывать обратный отсчёт;
  но решает часы демона (`None` = нечего отсчитывать).
- `Secret` идёт **непосредственно перед** `Accepted`: клиент, который его игнорирует, всё равно
  читает тот же вердикт.
- `SecretBytes` — отдельный тип с редактирующим `Debug` и `zeroize` при дропе
  (`secret_bytes.rs`, зависимость `zeroize`).
- `CodecError::Eof` — клиент отключился (например, пользователь нажал Enter).

### 2.4 Авторизация (`authz.rs`)

Чистая функция `authorize(caller_uid, caller_user, requested_user)`:

- **uid 0** — без ограничений (это путь `pam_gxfp.so`, работающего внутри root-стека PAM);
- **не-root** — принудительно переводится на **своё** имя пользователя, что бы ни запросил
  (так `gxfp-authctl` не может достать шаблоны соседа, даже назвав их);
- uid без записи в user-базе — `Denied`.

`PeerIdentity::from_stream` берёт uid/имя из `SO_PEERCRED` — клиентский `user` не считается доверенным.

### 2.5 Хендлеры

- **`handle_enroll.rs`** — модель «композит из фрагментов» (как Touch ID): палец больше сенсора
  (80×64), поэтому собирается по **проходам** «положить — провести — отпустить — перехватить».
  Вид сохраняется, только если палец реально **продвинулся** (одометрия кадр-к-кадру ≥ 11 px)
  и участок ещё не покрыт (широкий дедуп HSV-обобщённый NCC < `set_novelty`).
  Завершение: покрытие перестало расти (`enroll_dry_passes`), достигнут `set_max` или `enroll_max_passes`,
  либо таймаут повторного прижатия. Если набрано меньше `enroll_min_coverage` — **не сохраняем**
  (чтобы не затереть хороший шаблон слабым) и прямо говорим «проводите по всей подушечке».
  Пере-запись того же `(user, finger)` — **перезапись**, о чём предупреждает `Progress`.
  Итог: `SetTemplate` → `seal` → `store.put(user, finger, ciphertext)`.
- **`handle_verify.rs`** — грузит и расшифровывает **все** пальцы пользователя в плоский список видов,
  затем до `verify_attempts` попыток: «положить палец (n/N)» → гейт пальца → burst → fusion →
  quality gate → скоринг против всех видов параллельно (short-circuit `rayon::any`) →
  принять на первой попытке, прошедшей порог. Диагностика в `Progress`:
  `best/2nd/top5` и сколько видов прошло порог. Для `hybrid` сначала дешёвый эмбеддер, только
  потом дорогой NCC. Время ожидания пальца **отменяется**, если клиент закрыл сокет
  (`peer_gone` через неблокирующий `poll`).
- **`handle_list.rs` / `handle_delete.rs`** — перечисление/удаление меток пальцев.
- **`handle_secret.rs`** — `seal`/`clear` секрета (логирует только **длину**).
- **`release_secret`** (в verify) — расшифровка секрета **только после** успешного матча;
  ошибка TPM/отсутствие блоба **не ломают** аутентификацию, только пишут в лог.
- **`handle_dump.rs`** — отладочный дамп (не участвует в логине).

### 2.6 Хранилище шаблонов

- `sealed_template.rs`: `SetTemplate` → bincode → `encrypt` (AES-256-GCM, `gxfp-template-protection`)
  → `ProtectedBlob` → непрозрачные байты. Обратное — `open`. Любая ошибка кадрирования/аутентификации
  коллапсирует в единое «template unreadable», чтобы нельзя было отличить «не тот ключ» от «битый файл».
- `gxfp-secure-store` — порт `SecureTemplateStore` над непрозрачными блобами;
  `gxfp-secure-store-redb` — адаптер: ключ `(user, finger)` → блобы.
  redb — **адаптер**, а не модель; домен не знает про redb.
- Ключ — `gxfp-keystore`: порт `KeyProvider` + `KeyfileKeyProvider` (разработка/fallback;
  прод-путь по спеке — TPM2/linux keyring за тем же трейтом). Ключ **никогда** не выводится из сенсора.

### 2.7 TPM-секрет для связки ключей (`secret_store.rs`)

Зачем: отпечаток **не может быть** ключом (каждое касание даёт другие биты), поэтому
разблокировка `gnome-keyring` после логина по пальцу — это «хранить пароль и отдать его после матча».

- Пароль запечатывается `systemd-creds encrypt --with-key=host+tpm2` — нужны **и** root-only
  host key, **и** TPM этой машины. Свой крипто-код не пишется.
- Файл `gxfp.<user>.cred`, каталог `0700`, файл `0600`, запись через write-then-rename;
  имя пользователя валидируется (никаких `../`, точек в начале, пробелов, не-ASCII).
- Плейнтекст не касается диска (только через stdin/stdout `systemd-creds`), живёт в `SecretBytes`.
- Честная граница: кража/клон диска — бесполезно; **локальный root на этой машине** — может открыть
  (но он и так правит демон); LiveUSB = локальный root, а Secure Boot выключен, так что PCR-биндинг
  сейчас не помог бы.
- Тред-офф: подделка пальца теперь даёт пароль ключницы, а не просто сессию. После смены пароля
  логина `secret set` надо повторить (устаревший секрет просто не разблокирует — подсказка, не блокировка).

---

## 3. `pam_gxfp.so` — PAM-модуль (`crates/adapters/gxfp-pam`)

- `cdylib`+`rlib` (rlib — ради юнит-тестов чистой логики). `pam_sm_authenticate` + no-op `pam_sm_setcred`.
  Весь `unsafe`/FFI — в `pam_ffi.rs`, остальное `#![deny(unsafe_code)]`.
- Опции строки `pam.d`: `socket=<path>`; иначе env `GXFP_AUTHD_SOCKET`; иначе `/run/gxfp/authd.sock`.
  `authtok` — опт-ин: публиковать полученный секрет как `PAM_AUTHTOK`.
- Логика: получить `PAM_USER` → открыть сокет → `Verify { user, with_secret }` → читать кадры.
- **Отмена по Enter**: пока ждём, `poll` следит и за сокетом, и за управляющим терминалом;
  Enter → закрыть сокет → демон прерывает ожидание пальца → `PAM_AUTHINFO_UNAVAIL` (переход к паролю).
- Отображение (`ui.rs`, `tty_ui.rs`):
  - на терминале с ANSI (`TERM` не пуст и не `dumb`) — **одна живая строка**: спиннер, вердикт-глиф
    (✓/✗/⏎), обратный отсчёт `m:ss` из `Prompt.timeout_secs`, подсказка `Enter → password`.
    Намеренно **без** alt-screen, raw mode и скрытия курсора (`Ctrl-C` убьёт процесс раньше `Drop`);
    при любом выходе строка стирается. `NO_COLOR` уважается.
  - иначе — обычная PAM-беседа, по строке на сообщение, с подсказкой про Enter только если терминал есть.
- Маппинг вердиктов:

| Ситуация | PAM-код | Эффект в `sufficient` |
|---|---|---|
| Совпадение | `PAM_SUCCESS` | авторизован |
| Чистый не-матч | `PAM_AUTH_ERR` | переход к следующему методу |
| Enter/отмена | `PAM_AUTHINFO_UNAVAIL` | переход к паролю |
| Демон недоступен / ошибка | `PAM_AUTHINFO_UNAVAIL` | переход к паролю |
| Неизвестный пользователь | `PAM_USER_UNKNOWN` | переход к следующему |

- `authtok`: если секрет получен и `PAM_AUTHTOK` выставить не удалось — только `info`
  («could not pass the keyring secret on»), вердикт не меняется.

---

## 4. `gxfp-authctl` — CLI-клиент (непривилегированный)

`crates/apps/gxfp-authctl` · грамматика (`cli.rs`):

```
gxfp-authctl [--socket <path>] enroll --finger <name> [--user <u>] [--swipe]
gxfp-authctl [--socket <path>] verify [--user <u>]
gxfp-authctl [--socket <path>] list   [--user <u>]
gxfp-authctl [--socket <path>] delete --finger <name> [--user <u>]
gxfp-authctl [--socket <path>] secret set   [--user <u>] [--stdin]
gxfp-authctl [--socket <path>] secret clear [--user <u>]
```

- `--user` по умолчанию — текущий логин: `$USER`/`$LOGNAME`, иначе real uid из `/proc/self/status`
  → `/etc/passwd` (без libc, крейт `#![forbid(unsafe_code)]`). Демон этому не доверяет.
- Вывод (`exchange.rs`): `Prompt` → `>> текст (Ns)`, `Progress` → `.. текст`,
  `Accepted` → `OK` (0), `Rejected` → `no match` (1), `Error` → stderr (2). `verify` никогда не просит секрет.
- `secret set` (`secret_input.rs`): пароль через `systemd-ask-password --timeout=120` (echo off)
  или `--stdin`; один хвостовой `\n` срезается; пустой → ошибка. В аргументах пароль не передаётся.
- Unit-тесты: парсер, текущий пользователь, обмен по фейковому сокету.

Пользовательские сценарии (из `docs/FINGERPRINT_LOGIN.md`): enroll `--swipe` с подсказками
«положить — медленно провести — отпустить — перехватить»; `list`/`delete`; `verify` для подбора
порога по диагностике.

---

## 5. `gxfp-plaque` — Wayland-оверлей статуса (пользовательская сессия)

`crates/apps/gxfp-plaque` (вне workspace: ссылается на sibling-чекаут `obscur.kit_v2`).

- **Канал:** `gxfp-fp-events` — абстрактный unix datagram-сокет `\0gxfp-fp-events`, 1 байт на событие.
  Выбран абстрактный, потому что у root-демона и юзера нет общего пути. Отправка best-effort:
  нет слушателя — датаграмма молча теряется, аутентификацию это не задерживает. Однопользовательское допущение:
  один адрес, кто первый занялся — тот и получает.
- **События:** `PlaceFinger`, `Checking`, `Accepted`, `Rejected`, `End` (нет вердикта: отмена/таймаут/ошибка).
  Демон шлёт их через `VerifyBeacon`, у которого `Drop` гарантирует `End` на **любом** пути выхода.
- **Внешность:** layer-shell карточка в углу, namespace `gxfp-plaque` (Hyprland: `layerrule = blur, gxfp-plaque`).
  Карточка полупрозрачная (0.72), диск, кольцо-спиннер во время матчинга, иконка.
  Стили/анимации — в CSS-подобном stylesheet; палитра — из `~/.config/gxfp/palette.toml`, иначе дизайн-палитра +
  следующие за системой light/dark.
- **Настройка без правки юнита:** `~/.config/gxfp/plaque.env` (`EnvironmentFile=-`):
  `GXFP_PLAQUE_OUTPUT`, `_BG_ALPHA`, `_THEME=light|dark`, `_W/_H/_MX/_MY/_TRAVEL/_INSET`.
  Неизвестный выход — warning, дальше выбирает композитор.
- **Юнит:** user-scope `gxfp-plaque.service` (`PartOf=graphical-session.target`, `Nice=5`, idle = 0 % CPU,
  два фоновых потока: события и дедлайн вердикта).
- **Требования:** `zwlr_layer_shell_v1` (Hyprland/sway/KWin); на GNOME `AppError::LayerUnsupported`, без краша.
- **Тема Noctalia** — опт-ин (`packaging/noctalia/`): `palette.toml` (4 роли × dark/light),
  `gxfp-plaque.toml`, `render-palette.sh` + хук `colors_changed` в настройках Noctalia.
  Переключение light/dark перерисовки не требует (оба набора в одном файле); менять цвет — нужен хук.

---

## 6. Интеграция с системой: PAM-стеки

Референсные файлы ship-атся в `/usr/share/doc/gxfp-fingerprint/pam/`, **не** в `/etc/pam.d`
(неверный стек = блокировка входа).

| Поверхность | PAM-сервис | Файл | Форма |
|---|---|---|---|
| `sudo` | `sudo` | (документация) | `auth sufficient pam_gxfp.so` |
| polkit (hyprpolkitagent/lxqt) | `polkit-1` | `packaging/pam/polkit-1` | `sufficient` + `include system-auth` |
| Lock-screen Noctalia | `noctalia-lock` (через `$NOCTALIA_PAM_SERVICE`) | `packaging/pam/noctalia-lock` | `sufficient` + `include login` |
| Greeter (greetd) | `greetd` | `packaging/pam/greetd` | `[success=1 default=ignore] pam_gxfp.so authtok` + `substack` + `pam_gnome_keyring` |

Почему greeter не `sufficient`: успех закончил бы стек, и `pam_gnome_keyring` не разблокировал бы
ключницу. `success=1` перепрыгивает ровно один `substack system-local-login` и попадает на
keyring-модуль, который берёт опубликованный `PAM_AUTHTOK`. `substack`, а не `include` —
прыжки через `include` ненадёжны.

Noctalia-лок: `general.autoStartAuth = true` (ждать палец сразу, а не после первого нажатия),
`pam.abort()` при вводе, `allowPasswordWithFprintd = false` (он-то и занимает сенсор через `fprintd-verify`).
Всегда `sufficient`, никогда `required` — отказ сенсора/демона не должен блокировать пароль.
`GXFP_AUTHD_SOCKET`/`socket=` — куда смотреть модулю.

Порядок работ по документации: сначала `sudo` (с открытым root-шеллом), затем login/display-manager.

---

## 7. `gxfp-cli` — инструментальная поверхность

`crates/apps/gxfp-cli` (composition root). Подкоманды:

| Команда | Назначение | Пользовательский аспект |
|---|---|---|
| `capture` | один отпечаток за касание → `finger.pgm` | отладка сенсора |
| `stream` | GXF1-кадры в stdout (по умолчанию по появлению пальца; `--continuous`, `--fps`) | просмотр/запись |
| `enroll` / `verify` | офлайн или **без железа** (`--from-gxf1`) — bincode-шаблон | dev-инструмент |
| `provision-psk` | сгенерировать новый TLS-PSK, записать в MCU, сохранить файл | его вызывает `gxfp-psk-provision` |
| `recover` / `mcu-state` | расщепление MCU, оставшегося после Windows | восстановление |
| `psk-verify` / `psk-dump` | сравнить/дамп PSK сенсора | диагностика «MAC failed» |
| `sensor-dump` | MCU state, chip id, OTP, sweep регистров | глубокая диагностика |

Опт-ин диагностика (по умолчанию **выключено**): `--record-trace`, `--record-dataset`
(PGM + `.raw` + `manifest.json`, dev-only).

Требует root и PSK; собирать под пользователем, запускать бинарь под `sudo`
(не `sudo cargo run`).

Шаблоны `enroll` в CLI — **плейнтекст** (dev-артефакты), не защищённое хранилище.

---

## 8. GUI-пара `gxfp-gui` / `gxfp-gui-backend`

Два процесса, разделение привилегий:

- **`gxfp-gui-backend`** (root): держит сессию сенсора, говорит по stdin/stdout через `gxfp-gui-proto`
  (`GuiCommand` вверх: `StartEnroll{finger_id,out_path,target_fragments}`, `StartVerify{template_path}`,
  `Cancel`, `Quit`; `GuiEvent` вниз: `Ready{rows,cols}`, `Mode`, `Frame`, `FingerDown/Up`,
  `EnrollProgress{collected,target,last,coherence,keypoints}`, `EnrollComplete`, `Verdict{…,inliers,threshold,frames_passed/total}`,
  `Status`, `Error`, `Bye`). Кадры — с `u32` длиной + bincode.
- **`gxfp-gui`** (egui, без root/FFI): сам спавнит бэкенд и общается через его pipe:
  `gxfp-gui --finger-id right_index --template /tmp/gxfp.tpl -- sudo gxfp-gui-backend --psk-raw32 …`;
  без `--` читает события из stdin. Кнопки Enroll/Verify/Cancel, прогресс-bar, ACCEPT/REJECT,
  статистика кадра, отдельное окно 1:1 для мозаики.
- Политика бэкенда (`policy.rs`) повторяет те же ручки (matcher, burst, fuse, verify-attempts,
  `--verify-swipe` с `min_matches`/`min_rate` и т.д.).

**Статус:** исследовательский/демонстрационный инструмент; шаблоны — файлы, без шифрования.
Прод-путь логина — `gxfp-authd`, не этот GUI.

---

## 9. Пакетирование и развёртывание

### 9.1 Arch-пакет (`packaging/arch/PKGBUILD`)

- Один пакет `gxfp-fingerprint`: `gxfp-authd` (собрано с `--features embed`), `gxfp-authctl`, `gxfp-cli`,
  `pam_gxfp.so` (`/usr/lib/security/`), `gxfp-plaque` (внешний чекаут), helper-скрипты,
  system/user-юниты, udev-правило, модель эмбеддера `/usr/share/gxfp/embedder_real.onnx`,
  конфиг `/etc/gxfp/authd.toml` (`backup=`, путь к модели переписывается), DKMS-исходники драйвера,
  reference-PAM-файлы и документация в `/usr/share/doc/…`, автозагрузка модуля `modules-load.d/gxfp.conf`.
- Зависимости: `openvino`, `openvino-intel-npu-plugin`, `intel-npu-driver`, `level-zero-loader`, `dkms`.
  OpenVINO линкуется `dlopen`-ом — апгрейд рантайма не требует пересборки.
- `pkgver()` от git — пакет собирается из рабочего дерева, а не из tarball.
- `.install`: `dkms add/install`, `depmod`, `modprobe`, reload udev, `daemon-reload`;
  в посте — чек-лист (PSK → включить демон → enroll → PAM) и предупреждение, что NPU доступен группе `video`.
  При удалении данные (`/var/lib/gxfp`, `/etc/gxfp`) остаются.

### 9.2 Юниты и конфиги

- `gxfp-authd.service`: root, `Restart=on-failure` с геометрическим backoff (2 с → 5 мин,
  `RestartSteps=8`), `RuntimeDirectory=gxfp` `0755`, `StateDirectory=gxfp` `0700`,
  `Before=display-manager.service`. Сэндбоксинг намеренно мягкий (нужен `/dev/gxfp`).
- `gxfp-plaque.service`: user-scope, см. §5.
- `gxfp-monitor.service` + `gxfp-monitor.sh`: **пассивный** user-сэмплер: раз в `GXFP_MON_INTERVAL`
  пишет в CSV мгновенную CPU демона (по дельте jiffies), число потоков, RSS, число клиентов сокета, load1;
  дорогой детальный дамп (hot threads, socket peers, top) — **только при пике > `GXFP_MON_THRESH`**.
  «CPU-пик при `sock_clients=0`» — повод разбираться. Логи в `~/gxfp-logs`.
- `99-gxfp-npu.rules`: `accel*` → группа `video`, `0660` (иначе OpenVINO не увидит NPU).

### 9.3 Скрипты

- `gxfp-psk-provision.sh`: сам эскалирует sudo; останавливает демон (освобождает сенсор),
  запускает `gxfp-cli provision-psk`, `chmod 600`, поднимает демон обратно; дружелюбные ошибки
  (нет модуля, нет CLI). Триггер — «SSL verification of the message MAC failed» в логе.
- `rebuild-driver.sh`: пересборка `gxfp.ko` под текущее ядро, проверка vermagic, stop/rmmod/insmod,
  ожидание `/dev/gxfp`, рестарт демона, `reset-failed` (снять StartLimit); подсказки про pacman-hook/DKMS.
- `gxfp-extract-psk-from-windows.sh`, `packaging/windows/collect-gxfp-windows-data.ps1` — извлечение
  данных/PSK со стороны Windows (миграция с Windows-драйвера).

---

## 10. Безопасность пользовательской части (сводка)

**Что защищено и как**
- Шаблоны: AES-256-GCM, ключ в root-only `0600` файле, расшифровка только в демоне.
- Имена шаблонов: `(user, finger)`; не-root клиент сведён к своему пользователю.
- Секрет ключницы: `systemd-creds host+tpm2`, отдаётся **только root-пиру** и **только после матча**,
  `SecretBytes` зануляется, в логи идёт только длина. `SetSecret/ClearSecret` — только root.
- Пароль для `secret set` не передаётся в argv; из парсера не попадает в pure-логику.

**Осознанные ограничения**
- Локальный root может всё (шаблоны, ключ, патч демона) — граница не меняется.
- LiveUSB на этой машине = локальный root; Secure Boot выключен → PCR-биндинг сейчас не помог.
- После смены пароля логина секрет нужно обновить.
- Демон обрабатывает запросы последовательно: занятый сенсор = `EBUSY`/ожидание у остальных клиентов.
- GUI-пара и `gxfp-cli enroll/verify` — dev-инструменты с плейнтекст-шаблонами.
- Конфиг демона в `/etc/gxfp/authd.toml` — единственная точка настройки; флаги CLI/`ExecStart`
  перекрывают его для одного запуска.

---

## 11. Быстрый чек-лист развёртывания (по `FINGERPRINT_LOGIN.md`)

1. `sudo pacman -U gxfp-fingerprint-…` (DKMS соберёт `gxfp.ko`; нужны headers под текущее ядро).
2. `sudo gxfp-psk-provision` (PSK → `/var/lib/gxfp/psk_raw32.bin, 600`).
3. `sudo systemctl enable --now gxfp-authd`; при необходимости поправить `/etc/gxfp/authd.toml`
   (пакет уже ставит `hybrid` + NPU).
4. `gxfp-authctl enroll --finger right_index --swipe` → `list` → `verify` (подобрать `--verify-threshold`
   по строке `attempt k/N: best … 2nd … top5 …`).
5. `systemctl --user enable --now gxfp-plaque` (Wayland с layer-shell).
6. PAM: начать с `/etc/pam.d/sudo`, держа открытым root-шелл; затем polkit/lock/greeter
   по референсным файлам; при желании — `sudo gxfp-authctl secret set --user $USER` и `authtok`-стек greeter.
7. Диагностика: `gxfp-cli mcu-state|recover|psk-verify|sensor-dump`, `ping` демона через
   `gxfp-authctl verify`, `gxfp-monitor` для поиска «фоновой» нагрузки.

---

## 12. Карта «где что лежит»

| Компонент | Крейт / путь |
|---|---|
| Демон | `crates/apps/gxfp-authd` (`cli.rs`, `config_file.rs`, `policy.rs`, `serve.rs`, `authz.rs`, `peer.rs`, `handle_*.rs`, `session.rs`, `store_setup.rs`, `sealed_template.rs`, `secret_store.rs`, `embed_matcher.rs`, `finger.rs`, `respond.rs`) |
| Контракт демон↔клиент | `crates/apps/gxfp-auth-proto` |
| CLI-клиент | `crates/apps/gxfp-authctl` |
| PAM-модуль | `crates/adapters/gxfp-pam` (+ `pam_ffi.rs`, `ui.rs`, `tty_ui.rs`) |
| Оверлей | `crates/apps/gxfp-plaque`, канал — `crates/core/gxfp-fp-events` |
| GUI-пара | `crates/apps/gxfp-gui`, `crates/apps/gxfp-gui-backend`, `crates/diagnostics/gxfp-gui-proto` |
| Инструменты сенсора | `crates/apps/gxfp-cli` |
| Хранилище/ключ | `crates/biometric/gxfp-secure-store`, `gxfp-secure-store-redb`, `gxfp-keystore`, `gxfp-template-protection` |
| Пакет/сервисы | `packaging/arch/PKGBUILD`, `packaging/*.service`, `packaging/*.sh`, `packaging/pam/*`, `packaging/noctalia/*`, `packaging/99-gxfp-npu.rules` |
| Документы | `docs/FINGERPRINT_LOGIN.md`, `docs/GXFP5130_WORKSPACE_ARCHITECTURE.md` (§8, §11), `docs/MATCHING_DESIGN_V2.md` |
