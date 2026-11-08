# Rivet UX

Everything about how Rivet looks and behaves. The prototype is the reference for M1:
every Slint screen is checked against it at 800×480.

| Document | What it is |
|---|---|
| [Prototype](https://claude.ai/artifact/PnGxUHsDknJNEPP3s68usB) | 68 clickable screens 800×480 on a canvas; comments from reviews live there |
| [design-code.md](design-code.md) | Tokens, type, geometry, components and content rules. A screen that breaks one is a bug |
| [feature-inventory.md](feature-inventory.md) | Every function of KlipperScreen, GuppyScreen and HelixScreen and the one Rivet screen that owns it |
| [data-sources.md](data-sources.md) | Where each value on each screen comes from (Moonraker object/field or Rivet's own store) |
| [reference-uis.md](reference-uis.md) | How the reference screens adjust values, and what the AD5M under KlipperMod actually has |
| [research.md](research.md) | User needs, printer UIs studied, the visual language and the 12 UX rules |
| [audit-2026-11-08.md](audit-2026-11-08.md) | Full visual audit of all screens and what was fixed |
| [style/](style/) | Prototype sources: `gen.py` builds the typical screens from data, `extra.py` and `more.py` the special ones |

## Structure

Rail: **Главная · Управление · Инструменты · Настройки · Сообщения**, plus **Стоп** (emergency stop, one tap).

- **Главная** — last file, live temperatures, loaded spool; **Печать** with tuning, «во время печати», completion, runout.
- **Управление** — нагрев, движение, экструдер и заправка, вентиляторы, свет, скорость и поток, Z-offset, устройства.
- **Инструменты** — калибровки (карта стола, винты, Z-offset, вибрации, ремни, PID, PA, тензодатчик, TMC), макросы, консоль, история, лимиты, датчики, сервис, камера.
- **Настройки** — сеть, экран, блокировка, звук, язык и время, безопасность, принтеры, обновления, система.
- **Сообщения** — journal; macro prompts and Klipper/connection errors appear over any screen.

## Regenerating the prototype

```bash
cd docs/ux/style && python3 -c "import gen, extra; gen.build(); extra.build_extra()"
```

The generator writes `.dc.html` next to itself; publish them to the canvas to update the prototype.
