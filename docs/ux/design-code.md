# Rivet design code

The rules every screen follows. A screen that breaks one is a bug.
Screen: 800×480, landscape, finger at arm's length, ~8.5 px per mm.

## 1. Tokens

| Token | Value | Use |
|---|---|---|
| `bg` | `#0b0d10` | screen background |
| `rail` | `#0f1215` | navigation rail |
| `surface-1` | `#15181d` | cards, rows, tiles |
| `surface-2` | `#1f242b` | buttons inside a card, inactive segments, ± |
| `surface-3` | `#252a31` | keys, raised buttons |
| `well` | `#0b0d10` | value field inside a card |
| `text` | `#f4f6f8` | primary text, values |
| `text-2` | `#c7cdd4` | body copy |
| `text-dim` | `#98a1ac` | captions, units |
| `text-faint` | `#66707b` | section headers, placeholders |
| `accent` | `#2dd4bf` | primary action, selection, fills, changed values |
| `accent-ink` | `#042f2a` | text on accent |
| `accent-soft` | `rgba(45,212,191,0.13)` + `#5eead4` text | chips, changed markers |
| `heat` | `#fbbf24 → #f97316` | temperatures only |
| `danger` | `#fb7185` text on `rgba(251,75,94,0.14)`; solid `#e11d48` for the confirm button | stop, delete, errors |
| `warn` | `#fbbf24` / `#fcd34d` | attention, not error |

No other colours. No gradients except heat, accent fills of bars, and thumbnails.

## 2. Type (Inter)

| Role | Size / weight |
|---|---|
| Screen title | 26 / 800, −0.02em |
| Hero value (progress, temperature) | 56–76 / 800, tabular |
| Card value | 24–32 / 800, tabular |
| Row title | 17–18 / 700 |
| Body | 15–17 / 400–500, line-height 1.45 |
| Caption, unit | 13–14 / 600, `text-dim` |
| Section header | 13 / 700, uppercase, +0.04em, `text-faint` |

Units never sit next to a large number in the same line box; they go in the title line
(`Скорость  мм/с`) or under the number. Numbers are always `tabular-nums`, `nowrap`.

## 3. Geometry

- Spacing scale: 4, 6, 8, 10, 12, 16, 20, 24. Screen padding 16.
- Radii: 14–16 small controls, 18–20 rows and buttons, 22–24 cards, 28 dialogs.
- Touch targets: primary actions ≥ 76 px tall; any tappable ≥ 64×64; keyboard keys ≥ 50 px tall
  (only exception).
- One screen = one header row (64 px): back (64×64) · title · at most one action on the right.

## 4. Components

| Component | Rule |
|---|---|
| Rail | 88 px, 5 sections + Стоп; icon 28 + label 11/600; active = accent-soft fill |
| Hub tile | icon top-left, value or name bottom; live tiles show the value, tool tiles show the name |
| List row | 72 px min, optional 44 px icon tile, title + caption, value right, chevron |
| Toggle row | whole row is the switch; 60×34 switch on the right |
| Segmented | equal buttons 64 px tall, selected = accent fill |
| Range value | **vertical bar** (drag) + − / + under it; value above; never horizontal sliders |
| Exact number | − · value · + ; tap the value → numeric keypad |
| Text input | tap field → on-screen keyboard; never a bare input |
| Primary button | accent fill, 76–80 px, one per screen |
| Destructive | danger text on soft red; confirm dialog with solid red button |
| Dialog | dimmed backdrop 78%, 28 radius card, actions at the bottom, cancel left |
| Changed value | value turns accent + chip «↺ было X» that resets it; no prose |
| Scroll | lists that overflow show a visible scrollbar on the right |

## 5. Content

- Words a user says, not Klipper's: no G-code, API names, config section names in captions.
  The one exception is the console, macro list and the raw Klipper error, labelled as such.
- Every number on screen has a source in Moonraker or Rivet (`data-sources.md`); no invented data.
- Show a function only when the printer has it (capability-gated).
- Captions explain a consequence («Klipper перезапустится»), never repeat the title.
- Russian first; strings must fit when English is ~15% longer.

## 6. Behaviour

- Commands go out on release, not on every drag tick.
- Live values update in place; stale values are hidden, not shown.
- Destructive or irreversible actions always confirm. The one exception is E-stop: one tap stops at once (optional 1 s hold in Безопасность).
