# Проверка экспериментальной подмены GNOME Shell

Это способ испытания, а не подтверждённое исправление сбоя. Патч не
доказывает завершение серверной операции после отмены. Не применять после
обновления GNOME Shell без повторной проверки исходных ресурсов.

На планшете установлены `gnome-shell 1:51.1-1`, `glib2 2.90.1-1`.
Файл `/usr/lib/gnome-shell/libshell-51.so` имеет SHA-256
`5946714975b163d2bd7d8a4187f4771d667307e4415422f49326a54a72911914`.
Ресурсы `userVerifier.js` и `authServices.js` имеют SHA-256 соответственно
`9f9c3c0794ddcb17a2c02ff3c11b090221332a045016f937dee5bef6a76424af` и
`516748c7ab95c9da89066780e97a336f191db76aadddd67802b72f5361acc1fd`.
Проверить оба перед применением; патч должен примениться без смещений.

Установленный GLib проверен отдельным процессом: переменная
`G_RESOURCE_OVERLAYS=/org/gnome/shell/gdm/userVerifier.js=/etc/os-release`
подменила только этот ресурс, второй остался штатным. Пробный процесс не
менял систему. GLib принимает сопоставление `путь_ресурса=путь_файла`,
несколько сопоставлений разделяются двоеточием.

Для ограничения действия пользовательским экраном блокировки следует
создать у пользователя `alex` каталог `~/.config/systemd/user/` и в нём
drop-in `org.gnome.Shell@user.service.d/carosign-experiment.conf`:

```ini
[Service]
Environment=G_RESOURCE_OVERLAYS=/org/gnome/shell/gdm=/home/alex/.local/share/carosign-gnome-overlay/gdm
```

В указанном каталоге `gdm` должны лежать **только** `userVerifier.js` и
`authServices.js`, полученные применением `generation-limited-open.patch`
к извлечённым штатным ресурсам; каталог и файлы должны быть читаемы владельцем
сеанса. Штатные ресурсы и пакет не изменяются. Переменная обрабатывается при
старте процесса, поэтому существующая сессия не меняется. Unit планшета
`gnome-session@gnome.target` требует `org.gnome.Shell@user.service`, а GDM
запускает `org.gnome.Shell@gdm.service` от отдельной учётной записи: этот
пользовательский drop-in не затронет GDM. Не помещать drop-in в общий
`/etc/systemd/user/org.gnome.Shell@.service.d`.

После записи drop-in выполнить `systemctl --user daemon-reload` от `alex`,
затем начать новый графический сеанс. Применение к уже запущенному Shell не
происходит. Проверка: у нового PID `gnome-shell --mode=user` прочитать
`/proc/PID/environ` и найти `G_RESOURCE_OVERLAYS`; в журнале процесса GLib
должен сообщить `Mapped file ... as a resource overlay` для обоих файлов.
Одна переменная окружения без сообщений о подмене не доказывает загрузку.

Откат: удалить только упомянутый drop-in, выполнить от `alex`
`systemctl --user daemon-reload` и начать новый сеанс. После выхода из старого
сеанса удалить каталог подмены. Удаление drop-in не возвращает ресурсы уже
запущенного процесса. При возникновении проблем доступен вход по SSH для
отката; в этом испытании нельзя считать успехом только запуск Shell, нужно
отдельно проверить экран блокировки и вход паролем.

Документация GLib: https://gnome.pages.gitlab.gnome.org/gtk/gio/struct.Resource.html#overlays
