# Changelog

Each release's section is also its description on GitHub. [Русский — ниже в каждом разделе.]

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
