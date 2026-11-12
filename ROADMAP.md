# Roadmap

What is done, what comes next. [Русский — ниже.](#план)

## 0.1 alpha — now

The Adventurer 5M / 5M Pro on Klipper Mod: all screens, USB and ssh installers, updates through
Moonraker with roll-back, ready for Z-Mod. See [CHANGELOG.md](CHANGELOG.md).

## Beta

- Ten real prints in a row without the screen failing or disturbing the print; processor and memory
  measured during a print.
- Z-Mod: checked on a printer once Z-Mod offers Rivet.
- What alpha testers report.
- Several printers on one screen.
- Fine Z adjustment on a test layer; pressure advance tower (both need a test print file).
- Skew correction on the bed mesh screen, when the printer has it configured.

## Later

- **Adventurer 5X**: a MIPS processor (Rust supports it, but not as a regular target) and the
  4-colour IFS.
- Other Klipper printers and screen sizes other than 800×480.
- Forge-X.
- Live camera picture on the screen, if the processor allows it.
- Rivet on a separate Raspberry Pi or mini PC as a remote screen.

## Not planned

- OctoPrint, or firmware without Moonraker.
- A web interface: Fluidd and Mainsail do that.
- 3D G-code preview on a 128 MB printer.

## Limits it keeps (AD5M)

| | Target | Ceiling |
|---|---|---|
| Memory, idle | 10 MB | 15 MB |
| Memory, at most | 20 MB | 25 MB |
| Processor, idle | ~0 % | 1 % |
| Processor, printing | 5 % | 10 % |
| Program size | — | 16 MB |
| Start to first picture | 1.5 s | 3 s |

---

## План

**0.1 alpha — сейчас.** Adventurer 5M / 5M Pro на Klipper Mod: все экраны, установка с флешки и по
ssh, обновления через Moonraker с откатом, готовность для Z-Mod.

**Бета.**
- Десять настоящих печатей подряд без сбоев экрана и без влияния на печать; замер процессора и
  памяти во время печати.
- Z-Mod: проверка на принтере, когда Z-Mod добавит Rivet.
- То, что найдут тестеры альфы.
- Несколько принтеров на одном экране.
- Точная подстройка Z на тестовом слое; башня pressure advance (обеим нужен тестовый файл).
- Коррекция перекоса на экране карты стола, если она настроена у принтера.

**Позже.**
- **Adventurer 5X**: процессор MIPS (Rust его поддерживает, но не как обычную цель сборки) и IFS
  на 4 цвета.
- Другие принтеры на Klipper и экраны не 800×480.
- Forge-X.
- Живая картинка с камеры на экране, если позволит процессор.
- Rivet на отдельном Raspberry Pi или мини-ПК как удалённый экран.

**Не планируется.** OctoPrint и прошивки без Moonraker; веб-интерфейс (для этого есть Fluidd и
Mainsail); 3D-превью G-code на принтере со 128 МБ.

**Ограничения, которые держим (AD5M):** память в простое 10 МБ (потолок 15), максимум 20 МБ
(потолок 25), процессор в простое около 0 % (потолок 1 %), во время печати 5 % (потолок 10 %),
размер программы до 16 МБ, от запуска до первой картинки 1,5 с (потолок 3 с).
