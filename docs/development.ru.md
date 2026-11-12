# Разработка

Как собрать Rivet, запустить его на компьютере, проверить на принтере и выпустить релиз.
[English version](development.md).

## Сборка и запуск на компьютере

Нужен Rust stable ([rustup](https://rustup.rs)). Окно 800×480, как у принтера.

Без принтера, с поддельным Moonraker:

```bash
cargo run -p mock-moonraker          # терминал 1; команды print, pause, shutdown, ready…
cargo run -p rivet                   # терминал 2
```

С настоящим принтером по сети (на принтер ничего не ставится):

```bash
cargo run -p rivet -- --moonraker ws://IP_ПРИНТЕРА:7125/websocket --lang ru
```

`RUST_LOG=debug` — подробный лог. `--screen ИМЯ` — открыть нужный экран при запуске.

## Сборка для принтера

```bash
rustup target add armv7-unknown-linux-musleabihf
cargo printer       # псевдоним из .cargo/config.toml
```

Получается `target/armv7-unknown-linux-musleabihf/release/rivet`: статический, собран
линковщиком `rust-lld` из rustup, отдельный кросс-тулчейн не нужен. Дальше:

```bash
tools/swap-screen.sh rivet IP_ПРИНТЕРА    # запустить из /tmp до перезагрузки; `guppy` — вернуть
tools/install.sh IP_ПРИНТЕРА              # установить как экран принтера
tools/printer-shot.py IP_ПРИНТЕРА out/ home files   # снимки прямо из /dev/fb0
```

## Проверки

То, что CI запускает на каждый push и pull request; запускайте перед push:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p rivet -p fbdev-platform --no-default-features --features fbdev --all-targets -- -D warnings
cargo test --workspace
python3 tools/extract-tr.py --check      # у каждой строки на экране есть русский перевод
python3 tools/gen-forms.py --check       # сгенерированные списки настроек совпадают со спецификацией
cargo run -p ui-snapshot -- snapshots    # каждый экран в PNG 800×480
```

Задача для принтера собирает `cargo printer`, проверяет размер (≤ 16 МБ) и запускает программу
под `qemu-arm`.

## Как устроен код

| Путь | Что |
|---|---|
| `crates/moonraker` | JSON-RPC Moonraker через websocket, с переподключением |
| `crates/model` | состояние принтера, собранное из обновлений Moonraker |
| `crates/app` | программа `rivet`: движок, который говорит с Moonraker, настройки, файлы, превью, Wi-Fi |
| `crates/fbdev-platform` | Slint прямо на кадровом буфере Linux и evdev: отрисовка, тач, подсветка, сон экрана |
| `crates/ui-snapshot` | рисует любой экран в PNG (и сырые картинки кадрового буфера для установщика с флешки) |
| `crates/mock-moonraker` | поддельный Moonraker со сценариями, чтобы работать без принтера |
| `ui/` | экраны на Slint; `ui/kit` — общие элементы; `ui/generated` делает `tools/gen-forms.py` |
| `i18n/ru` | русский перевод (gettext `.po`, вшит в программу) |
| `assets/fonts` | Inter, урезанный до латиницы и кириллицы скриптом `tools/subset-fonts.sh` |
| `docs/design` | дизайн: страницы прототипа, правила оформления, откуда берётся каждое значение |
| `tools/` | установщики, упаковка релиза, генераторы, помощники для принтера |

Движок работает в своём потоке (tokio, однопоточный) и отдаёт интерфейсу готовые значения; поток
интерфейса только рисует и читает тач. На принтере Rivet рисует программным рендером Slint в память
и копирует в кадровый буфер только изменённый прямоугольник, переключая две страницы.

**Тач.** События из evdev (`ts_uinput` на Klipper Mod или «сырая» панель через `pointercal` tslib)
проходят: калибровку пользователя по девяти точкам; короткое «приземление», пока палец
устанавливается; доводку до кнопки (касание мимо всех кнопок уходит к ближайшей в пределах 32 px);
решение «нажатие или протяжка» (14 px, два отсчёта); нажатие, которое остаётся на своей кнопке,
пока под ним ничего не прокручивается. Всё это — в `crates/fbdev-platform/src/{platform,touch_state,snap}.rs`,
с тестами на записях настоящих пальцев на AD5M.

**Экраны следуют дизайну.** Списки настроек генерируются из страниц прототипа в
`docs/design/style` скриптом `tools/gen-forms.py` (спецификация — `ui/spec/forms.toml`), иконки —
`tools/gen-icons.py`. `tools/compare-prototype.py` сравнивает отрисованные экраны с прототипом
попиксельно. Значения на экранах берутся только из Moonraker ([data-sources](design/data-sources.md));
ничего не выдумывается.

## Переводы

Строки в `.slint` — `@tr("English")`. Русский вариант добавляется в `i18n/ru/LC_MESSAGES/rivet.po`;
`tools/extract-tr.py --check` показывает, чего не хватает.

## Как выпустить релиз

С релизов обновляются принтеры, поэтому правила ниже обязательны.

1. В `main`, при зелёном CI: поставьте `version` в `Cargo.toml` (например, `0.1.1`) и добавьте в
   `CHANGELOG.md` раздел с заголовком ровно `## v0.1.1-alpha` (из него берётся описание релиза).
2. Закоммитьте, поставьте тег и отправьте его:

   ```bash
   git tag v0.1.1-alpha
   git push origin v0.1.1-alpha
   ```

3. `.github/workflows/release.yml` собирает программу, запускает `tools/package-release.sh` и
   публикует релиз с файлами:

   | Файл | Для чего |
   |---|---|
   | `rivet-ad5m.zip` (первым) | обновления: Moonraker на Klipper Mod берёт первый файл релиза, Z-Mod — файл с именем из `release_info.json` |
   | `rivet-<тег>-usb.zip` | установка с флешки |
   | `rivet-<тег>-usb-remove.zip` | удаление с флешки |
   | `rivet-<тег>-installer.tar.gz` | установка по ssh |

4. Проверьте на принтере: в Настройки › Обновления видна новая версия, «Обновить экран» её ставит,
   экран возвращается.

Нельзя:

- переименовывать `rivet-ad5m.zip` или загружать что-то раньше него: его ищет каждый
  установленный Rivet;
- помечать релиз как pre-release: Z-Mod (`channel: stable`) его не увидит;
- удалять опубликованный релиз: откат в Moonraker скачивает предыдущий;
- ставить тег, не совпадающий с версией в `Cargo.toml` (workflow откажется).

`tools/package-release.sh ВЕРСИЯ ПАПКА` делает те же файлы локально — чтобы проверить флешку до
тега.

## Репозиторий

Ветки: `main` всегда собирается и проходит CI. Работа идёт в ветке через pull request.
Сообщения коммитов в стиле Conventional Commits (`feat(touch): …`, `fix(update): …`).
