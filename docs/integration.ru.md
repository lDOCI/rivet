# Rivet как один из экранов вашей прошивки

Для авторов прошивок — Z-Mod, Forge-X, своих образов, — которые хотят предлагать Rivet рядом
с GuppyScreen или HelixScreen. [English version](integration.md).

Собственные установщики Rivet (флешка, `tools/install.sh`) работают только на Klipper Mod и
в других местах отказываются запускаться. В вашей прошивке Rivet ставит, запускает и
останавливает сама прошивка; здесь всё, что ей для этого нужно знать. Вопросы —
[в issues](https://github.com/lDOCI/rivet/issues).

## Коротко

1. Пусть менеджер обновлений Moonraker скачивает `rivet-ad5m.zip` из последнего релиза
   `lDOCI/rivet` на GitHub в выбранную вами папку (`type: web`).
2. Запускайте и останавливайте экран скриптом `S80rivet`, который лежит в том же архиве.
3. Перед запуском останавливайте другие экраны и никогда не запускайте два сразу.

## Релиз

В каждом релизе [lDOCI/rivet](https://github.com/lDOCI/rivet/releases) **первым** идёт файл с
именем ровно `rivet-ad5m.zip`. Имя от релиза к релизу не меняется. Релизы никогда не помечаются
как pre-release, поэтому `channel: stable` (на GitHub это «latest») видит каждый из них.

| Файл в архиве | Что |
|---|---|
| `rivet` | программа: статическая, `armv7-unknown-linux-musleabihf`, без разделяемых библиотек, шрифты и переводы внутри |
| `rivet.sha256` | её контрольная сумма в формате `sha256sum` |
| `release_info.json` | `{"project_name": "rivet", "project_owner": "lDOCI", "version": "v0.1.0-alpha", "asset_name": "rivet-ad5m.zip"}` |
| `S80rivet` | скрипт запуска и остановки со сторожем; работает из любой папки, где лежит |

Остальные файлы релиза (`rivet-<версия>-usb.zip`, `…-usb-remove.zip`, `…-installer.tar.gz`) —
установщики для Klipper Mod, их можно не трогать.

## Менеджер обновлений

```ini
[update_manager rivet]
type: web
channel: stable
repo: lDOCI/rivet
path: /srv/rivet
persistent_files:
    rivet.good
    rolled-back
```

`path` — любая папка. До первой установки Moonraker нужен там `release_info.json`; положите его
со старой версией, тогда первая же проверка предложит настоящую:

```json
{"project_name":"rivet","project_owner":"lDOCI","version":"v0.0.1","asset_name":"rivet-ad5m.zip"}
```

`persistent_files` сохраняет состояние сторожа между обновлениями: `rivet.good` — последняя
рабочая версия, `rolled-back` — отметка, что новая не запустилась.

После того как Moonraker поставил обновление, запущенный Rivet получает
`notify_update_response` и завершается с кодом 0; `S80rivet` запускает новую версию.
Перезапускать вам ничего не нужно.

## Запуск и остановка

```sh
/srv/rivet/S80rivet start     # возвращается сразу; сторож работает в фоне
/srv/rivet/S80rivet stop      # возвращается, когда экран освобождён
/srv/rivet/S80rivet restart
```

Сделайте ссылку в `/etc/init.d`, если ваша прошивка запускает экраны оттуда, или вызывайте из
своего переключателя экранов.

Что делает сторож:

- запускает Rivet и перезапускает, если тот завершился (через 1 с, при повторных быстрых
  падениях — до 60 с; выход с кодом 0 — это намеренный перезапуск, он не считается);
- перед каждым запуском сверяет программу с `rivet.sha256`;
- после 30 с работы сохраняет копию как `rivet.good`; новая программа, которая дважды
  завершилась в первые 30 с или не совпала с контрольной суммой, заменяется на `rivet.good`, и
  записывается `rolled-back`;
- при остановке возвращает кадровый буфер на первую страницу (см. ниже).

Папки `S80rivet` выбирает так:

| | Z-Mod (есть `/opt/config/mod_data`) | Klipper Mod (`/root/printer_data/config`) | иначе |
|---|---|---|---|
| программа | папка, где лежит `S80rivet`, если в ней есть `rivet`; иначе `/opt/rivet`; `RIVET_DIR` переопределяет | так же | так же |
| `rivet.toml`, `rivet-touch.json` | `/opt/config/mod_data` | `/root/printer_data/config` | папка программы |
| логи | `/opt/config/mod_data/log/rivet.log`, `rivet-console.log` | `/root/printer_data/logs/…` | папка программы |
| язык | из `mod_data/lang.cfg` Z-Mod, если в `rivet.toml` не задан свой | системный `LANG` | системный `LANG` |

Сторож — `rivet-watchdog`, маленькая программа рядом с `rivet` (в старых релизах это был цикл в
`S80rivet`). PID-файлы: `/run/rivet.pid` (сторож) и `/run/rivet.child.pid` (Rivet).
`killall rivet-watchdog rivet` останавливает оба, как `S80rivet stop`; один `killall rivet`
перезапускает Rivet.

Rivet регистрирует у Moonraker удалённый метод `rivet_screen` с одним аргументом `action`: `off`
(подсветка выключена до касания), `on`, `restart`. Макрос вызывает его так:
`{action_call_remote_method("rivet_screen", action="off")}`; установщики Rivet добавляют такой
макрос, `RIVET_SCREEN ACTION=…` ([tools/install/rivet.cfg](../tools/install/rivet.cfg)).

## Что Rivet нужно от системы

| | По умолчанию | Переопределить |
|---|---|---|
| Кадровый буфер | `/dev/fb0`, 32 бита (XRGB/XBGR) или RGB565, 800×480; две страницы, если драйвер умеет panning | `RIVET_FB=/dev/fbN` |
| Тач | первое evdev-устройство с абсолютными X/Y, предпочтительно `ts_uinput` (tslib) | `RIVET_TOUCH=/dev/input/eventN` |
| «Сырая» панель | если устройство не от tslib, его значения проходят через линейную калибровку tslib из `$TSLIB_CALIBFILE`, `/etc/pointercal` или `/tmp/pointercal` (копия Z-Mod) | `RIVET_TOUCH_SWAP_XY`, `…_INVERT_X`, `…_INVERT_Y` = `1` |
| Поворот | нет | `RIVET_ROTATION=90` (`0`, `90`, `180`, `270`) |
| Подсветка | `/sys/class/backlight/*`, иначе `/usr/bin/backlight` из Klipper Mod, иначе ioctl `/dev/disp` Allwinner (Z-Mod) | — |
| Moonraker | `ws://127.0.0.1:7125/websocket` | `--moonraker URL`, `RIVET_MOONRAKER`, `moonraker` в rivet.toml |
| Папка настроек | см. таблицу выше | `RIVET_CONFIG_DIR`, `--config ФАЙЛ` |
| Wi-Fi (необязательно) | управляющий сокет `wpa_supplicant` в `/var/run/wpa_supplicant` или `/run/wpa_supplicant`; нужен root | — |

На AD5M Rivet работает от root: это нужно для кадрового буфера, тача, подсветки и Wi-Fi.

**Страницы кадрового буфера.** Rivet рисует в одну страницу и показывает её сдвигом (panning),
по очереди. Экран, запущенный после него и запоминающий смещение при старте (так делает
LVGL-драйвер GuppyScreen), рисовал бы в невидимую страницу. Сторож сбрасывает смещение, когда
останавливается — через `S80rivet stop` или простым `kill $(cat /run/rivet.pid)`; если гасите
Rivet как-то иначе, сделайте это сами: `echo 0,0 > /sys/class/graphics/fb0/pan`.

## Что Rivet записывает

| Где | Что |
|---|---|
| папка настроек | `rivet-touch.json` после калибровки тача (`rivet.toml` только читается) |
| папка логов | `rivet.log` (ротация 512 КБ × 3), `rivet-console.log` (обрезается на 256 КБ) |
| папка программы | `rivet.good`, `rolled-back` (сторож) |
| база Moonraker, раздел `rivet` | настройки, изменённые на экране |
| `/tmp` | сокет `rivet-wpa-<pid>`, пока идёт разговор с `wpa_supplicant` |

Чтобы удалить Rivet полностью: удалите папку программы, эти файлы, секцию менеджера обновлений
и ключи раздела `rivet` в базе.

## Макросы Klipper, которыми он пользуется, если они есть

Rivet работает с любым конфигом Klipper и показывает то, что есть у принтера. На AD5M он
дополнительно пользуется этими макросами, если они есть, и прячет строки, если их нет:
`LOAD_FILAMENT`, `UNLOAD_FILAMENT`, `SET_PAUSE_AT_LAYER`, `SET_PAUSE_NEXT_LAYER`,
`LOAD_CELL_TARE`, `AUTO_SHUTDOWN`, `REBOOT_STOCK_SYSTEM`, `M300`.

## Z-Mod

То же, что Z-Mod делает для HelixScreen, только для Rivet:

- `.shell/plugman/plugins.json`:

  ```json
  "rivet": {"type": "web", "repo": "lDOCI/rivet", "path": "/srv/rivet",
            "persistent_files": ["rivet.good", "rolled-back"],
            "var": "rivet", "var_swap": "guppy",
            "check_files": ["rivet", "S80rivet"], "printers": ["FF5M"]}
  ```

- `start.sh`: если `/srv/rivet/release_info.json` нет, положить его как выше;
- переключатель экранов (`S80guppyscreen`, `zdisplay.sh`, `stop.sh`): при `rivet = 1` запускать и
  останавливать `/srv/rivet/S80rivet` там же, где `S80helixscreen`, и останавливать Rivet
  (`S80rivet stop`) там, где гасятся остальные экраны.

Предложено в Z-Mod: [ghzserg/z_ff5m#16](https://github.com/ghzserg/z_ff5m/pull/16) (плагины, `start.sh`,
`S80rivet`, `S80guppyscreen`, `zdisplay.sh`), [ghzserg/base#19](https://github.com/ghzserg/base/pull/19)
(`DISPLAY_OFF RIVET=1`) и [ghzserg/zmod#799](https://github.com/ghzserg/zmod/pull/799) (документация).

## Forge-X

Пока не поддерживается. У Forge-X свой chroot и своё управление экранами; понадобятся те же
три вещи (секция менеджера обновлений, `S80rivet`, остановка другого экрана). Не проверялось.
