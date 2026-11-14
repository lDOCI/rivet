# Changelog

Each release's section is also its description on GitHub. [Русский — ниже в каждом разделе.]

## v0.1.5-alpha

- Brightness at 100 % switched the screen off (the panel goes dark at the top of the driver's
  scale); 100 % is now just below it, with no visible difference.
- PID: the nozzle's and the bed's temperatures and the part fan (in %) are each set, also for
  Both, as in Fluidd.
- Checking for updates shows that it runs and what it found.

**Русский.** Яркость 100 % выключала экран (у панели край шкалы драйвера); теперь 100 % чуть
ниже края, разницы на глаз нет. PID: температура сопла, температура стола и обдув (в %)
задаются отдельно, в том числе для «Оба», как во Fluidd. Проверка обновлений показывает, что
идёт, и что нашла.

## v0.1.4-alpha

- Z-Mod: the part fan is `fanM106`, which Z-Mod's M106 drives. Rivet showed Klipper's `fan` as the
  part fan and `fanM106` as one more fan, so turning `fanM106` did not move the part fan's bar. It
  is now the part fan, shown once.
- Motion: the big buttons move by 10, 50 or 100 mm (the step button in the corner), to run an axis
  end to end after greasing it.

**Русский.** Z-Mod: обдув модели — это `fanM106`, им управляет M106 Z-Mod. Rivet показывал
обдувом `fan` Klipper, а `fanM106` отдельным вентилятором, поэтому `fanM106` не двигал полоску
обдува. Теперь он и есть обдув, показан один раз. Движение: большие кнопки двигают на 10, 50 или
100 мм (кнопка шага в углу), чтобы прогнать ось из конца в конец после смазки.

## v0.1.3-alpha

- Files from OrcaSlicer with several filaments: the material and the filament's name showed as a raw
  list (`["PETG", "PETG"]`, `\u041d…`); now "PETG" and the name.
- Language: with none chosen in Rivet, it follows Fluidd's or Mainsail's (Klipper Mod has no system
  language), instead of English.
- Time zone: Klipper Mod's zone was never recognised, so the clock stayed hidden; it is now, and it
  can be chosen in Settings › Language and time.
- `RIVET_SCREEN ACTION=OFF|ON|RESTART` from the console of Fluidd or Mainsail.
- PID calibration of both heaters with one Start (the nozzle, then the bed). The screen no longer
  goes to sleep while a calibration runs, and a long one (a bed PID test) is no longer reported
  finished after 10 minutes while Klipper still runs it.
- The Files screen takes 12 MB instead of 17: card previews are decoded at the size the card shows.
- A small watchdog program (`rivet-watchdog`, 0.3 MB instead of 1.6) instead of a shell loop that
  woke every 5 seconds; it comes with updates, together with the start script.

**Русский.** Файлы OrcaSlicer с несколькими филаментами: материал и название филамента
показывались сырым списком (`["PETG", "PETG"]`, `\u041d…`); теперь «PETG» и название. Язык: если
он не выбран в Rivet, берётся язык Fluidd или Mainsail (у Klipper Mod нет системного языка), а не
английский. Часовой пояс: пояс Klipper Mod не распознавался, и часы были скрыты; теперь
распознаётся и выбирается в Настройки › Язык и время. `RIVET_SCREEN ACTION=OFF|ON|RESTART` из
консоли Fluidd или Mainsail. Калибровка PID обоих нагревателей одной кнопкой (сопло, потом
стол); экран не засыпает во время калибровки, а долгая калибровка (PID стола) больше не
считается законченной через 10 минут, пока Klipper её ещё делает. Экран «Файлы» занимает 12 МБ
вместо 17: превью карточек распаковываются в размере карточки. Маленький сторож (`rivet-watchdog`, 0,3 МБ вместо 1,6) вместо цикла на shell, который
просыпался каждые 5 секунд; он приходит с обновлениями вместе со скриптом запуска.

## v0.1.2-alpha

- Z-Mod: the screen's backlight stayed off. Z-Mod has its own `backlight` command in the place
  where Klipper Mod keeps another one, taking percent instead of 0–255; Rivet now tells them apart
  by their answer and, on Z-Mod, drives the display's backlight directly.

**Русский.** Z-Mod: подсветка экрана оставалась выключенной. У Z-Mod своя команда `backlight` там
же, где у Klipper Mod другая, и она принимает проценты, а не 0–255; теперь Rivet различает их по
ответу и на Z-Mod управляет подсветкой дисплея напрямую.

## v0.1.1-alpha

- Pause at layer: the row was missing during a print whose slicer writes no layer numbers
  (OrcaSlicer by default). Rivet tells Klipper the layer count from the file, but the one it
  sent as the file loaded could be lost; it now checks and sends it again until Klipper has it.
- Installing from a USB stick: a stick with a partition table may be seen too late by Klipper
  Mod; the instructions now say to format it without one.

**Русский.** Пауза на слое: строки не было во время печати, если слайсер не записал номера
слоёв (OrcaSlicer по умолчанию). Rivet сообщает Klipper число слоёв из файла, но сообщение,
отправленное в момент загрузки файла, могло потеряться; теперь он проверяет и повторяет, пока
Klipper его не запомнит. Установка с флешки: флешку с таблицей разделов Klipper Mod может не
успеть увидеть; в инструкции теперь сказано, как отформатировать её без таблицы.

## v0.1.0-alpha

First public release, for the Flashforge Adventurer 5M / 5M Pro with Klipper Mod.

- Every screen of a printer screen: printing and tuning, files with previews and USB, heating,
  motion, extruder and filament wizard, fans, light, calibrations (bed mesh, screws, input
  shaper with graphs, belts, PID, pressure advance, load cell, TMC), macros, console, history,
  sensors, maintenance, drying, Spoolman, settings (Wi-Fi, screen, lock, sound, language, themes,
  updates, system). Rows appear only for what the printer has.
- Touch made for the AD5M's resistive panel: taps beside a button go to it, a press stays on its
  button, nine-point calibration on the screen, a touch test screen.
- Install and remove from a USB stick, or over ssh.
- Updates from GitHub through Moonraker (the screen, Fluidd or Mainsail), with a checksum check and
  an automatic return to the last version that worked.
- Ready for Z-Mod to offer as one of its screens ([docs/integration.md](docs/integration.md)).
- English and Russian.

**Русский.** Первый публичный релиз для Flashforge Adventurer 5M / 5M Pro с Klipper Mod.
Все экраны экрана принтера: печать и подстройка, файлы с превью и флешкой, нагрев, движение,
экструдер и мастер заправки, вентиляторы, свет, калибровки (карта стола, винты, input shaper с
графиками, ремни, PID, pressure advance, тензодатчик, TMC), макросы, консоль, история, датчики,
обслуживание, сушка, Spoolman, настройки (Wi-Fi, экран, блокировка, звук, язык, темы, обновления,
система); строки появляются, только если у принтера это есть. Тач под резистивную панель AD5M:
касания рядом с кнопкой попадают в неё, нажатие остаётся на своей кнопке, калибровка по девяти
точкам на экране, тест касаний. Установка и удаление с флешки или по ssh. Обновления с GitHub
через Moonraker (с экрана, из Fluidd или Mainsail) с проверкой контрольной суммы и
автоматическим возвратом к последней рабочей версии. Готов к тому, чтобы Z-Mod предлагал его
как один из своих экранов ([docs/integration.ru.md](docs/integration.ru.md)). Русский и английский.
