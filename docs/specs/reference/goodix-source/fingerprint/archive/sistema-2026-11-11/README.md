# Спецификация системы отпечатка: Си-версия и стоковый HAL

⚠️ **Как читать.** Это описание того, что уже РАБОТАЛО: ядерные модули и
демон на C (наш путь) и стоковый Android-слой (`libgf_hal.so`, `libgf_ca.so`,
`libQSEEComAPI.so`, `libdrmfs.so`, `librpmb.so`, `libion`) — глазами
декомпиляции. Rust-клиент обязан делать ровно это, не больше и не меньше.

Пометки происхождения:
* ✅SRC — прочитано в исходнике нашего C (`linux-rom/fingerprint/…`);
* ✅BIN — прочитано в двоичном стоке, адрес указан;
* ✅MSR — снято замером на планшете;
* ❌ — неизвестно, требует разбора.

Части:

1. [00 — Как устроено целиком](00-obzor.md) — слои, кто кого зовёт, где границы.
2. [01 — Ядро: шим `/dev/qseecom`](01-yadro-shim.md)
3. [02 — Ядро: proba (загрузка, питание, IRQ)](02-yadro-proba.md)
4. [03 — Ядро: слушатель TZ](03-yadro-slushatel.md)
5. [04 — Демон: FS](04-demon-fs.md)
6. [05 — Демон: GPFS](05-demon-gpfs.md)
7. [06 — Демон: RPMB](06-demon-rpmb.md)
8. [07 — HAL стока: состояния и команды](07-hal-sostoyaniya.md)
9. [08 — HAL стока: шина TZ `libgf_ca`](08-hal-libgf-ca.md)
10. [09 — QSEECom, ION, свойства Android](09-android-sloj.md)
11. [10 — Что обязан повторить Rust](10-rust-dolzhen.md) — сводка требований
12. [12 — Заголовок кадра: поля и смещения](12-zagolovok-kadra.md)
13. [13 — Событие: разбор по декомпиляции](13-sobytie-razbor.md) — главное
14. [11 — Что осталось неизвестным](11-otkrytye.md)
15. [14 — Команды HAL: полный перечень](14-komandy.md)
16. [15 — Таблица `g_hal_function`](15-tablica-hal-function.md)
17. [16 — Полная последовательность HAL](16-posledovatelnost.md)

## Третий том — что есть в стоке и в Си

Разделы 17–18 написаны **без оглядки на Rust**: только сток и наш C.

18. [17 — Сток и Си: жизненный цикл HAL](17-stok-i-si.md)
19. [18 — Сток: службы QSEE и Android](18-stok-sluzhby.md)
20. [19 — Сток: навигация, uinput, таймеры](19-stok-nav-i-tajmery.md)
21. [20 — Сток: навигация и uinput целиком](20-nav-i-uinput.md)
22. [21 — Сток: поворот жеста и служебные команды](21-povorot-i-sluzhebnye.md)
23. [22 — Сток: управляющие команды](22-upravlyayushchie-komandy.md)
24. [23 — Сток: модель, MT_BASE, дампы](23-model-i-mtbase.md)
25. [24 — Именованные таблицы: где лежат и как читать](24-imena-tablic.md)
26. [25 — Имена режимов, операций, навигации, ошибок — ПРОЧИТАНО](25-imena-svereno.md)
27. [26 — Сток: питание, сброс и тактирование](26-pitanie-i-takt.md)
28. [27 — Драйвер ядра стока: питание, сброс, IRQ](27-drajver-yadra-stoka.md)
29. [28 — `0x8958` и `0xc6c0`: что это за числа](28-zaderzhki-i-hz.md)
30. [29 — `libgf_ca`: шифрование канала](29-libgf-ca.md)
31. [30 — Сток: жизнь файлов и шаблонов](30-zhizn-fajlov-i-shablonov.md)
32. [31 — Сводка состояния разбора](31-svodka-sostoyaniya.md)
33. [32 — Чип, серия и выбор весов](32-chip-i-modeli.md)
34. [33 — Таблицы переходов: как читать](33-tablicy-perehodov.md)
35. [34 — Адреса: файл и образ (поправка)](34-adresa-fajl-i-obraz.md)
36. [35 — Выбор весов: правильный заход](35-vybor-vesov.md)
37. [36 — Выбор весов вида 0: прочитано до конца](36-vybor-vesov-prochitano.md)
38. [37 — `hal_milan_select_model` по дизассемблеру](37-select-model-dizassembler.md)
39. [38 — `libgf_ca`: порядок сеанса](38-libgf-ca-poryadok.md)
40. [39 — Веса: полный перечень символов](39-vesa-perechen.md)
41. [40 — Что осталось: пробелы разбора](40-chto-ostalos.md)
42. [41 — Пробел №1 закрыт: веса для 0x2e](41-vesa-dlya-0x2e.md)
43. [42 — Пробел №3: ASPDLWeight…PM](42-pm-vesa.md)
44. [43 — Пробел №4: asp_template_*](43-asp-template.md)
45. [44 — Пробелы №5 и №6](44-probely-5-6.md)

## Договорённость о замерах

⚠️ Замеры на устройстве — **когда перейдём к реализации**. В разделах
00–31 нет ни одного «нужно замерить, чтобы дописать»; всё, что не
прочитано в двоичном, помечено как «не разобрано» и ждёт разбора, а не
железа.

## Как получен материал

Модули ядра читаются как есть (`spinel_qseecom.c`, `spinel_fp_proba.c`,
`spinel_qsee_listener.c`); библиотеки — Ghidra (`/opt/ghidra/support/analyzeHeadless`)
и `aarch64-linux-gnu-objdump`. Разбор идёт от уже собранного материала:
`specs/stock/fingerprint/RAZBOR.md`, `specs/own/fingerprint/TZ-TRANSPORT-AUDIT.md`,
`linux-rom/fingerprint/hal-harness/staging/libgf_hal.so`.
