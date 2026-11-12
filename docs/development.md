# Development

Building Rivet, running it on a computer, testing it on a printer, and making a release.
[Русская версия](development.ru.md).

## Build and run on a computer

Needs Rust stable ([rustup](https://rustup.rs)). The window is 800×480, like the printer.

Without a printer, with the fake Moonraker:

```bash
cargo run -p mock-moonraker          # terminal 1; type print, pause, shutdown, ready…
cargo run -p rivet                   # terminal 2
```

Against a real printer (nothing is installed on it):

```bash
cargo run -p rivet -- --moonraker ws://PRINTER_IP:7125/websocket --lang ru
```

`RUST_LOG=debug` for more log. `--screen NAME` opens a given screen at start.

## Build for the printer

```bash
rustup target add armv7-unknown-linux-musleabihf
cargo printer       # alias in .cargo/config.toml
```

Gives `target/armv7-unknown-linux-musleabihf/release/rivet`: static, linked by rustup's
`rust-lld`, so no cross toolchain is needed. Then:

```bash
tools/swap-screen.sh rivet PRINTER_IP    # run it from /tmp until reboot; `guppy` goes back
tools/install.sh PRINTER_IP              # install as the printer's screen
tools/printer-shot.py PRINTER_IP out/ home files   # pictures straight from /dev/fb0
```

## Checks

What CI runs on every push and pull request; run them before pushing:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p rivet -p fbdev-platform --no-default-features --features fbdev --all-targets -- -D warnings
cargo test --workspace
python3 tools/extract-tr.py --check      # every on-screen string has a Russian translation
python3 tools/gen-forms.py --check       # generated settings lists match their spec
cargo run -p ui-snapshot -- snapshots    # every screen rendered to PNG, 800×480
```

The printer job builds with `cargo printer`, checks the size (≤ 16 MB) and starts the binary under
`qemu-arm`.

## How the code is laid out

| Path | What |
|---|---|
| `crates/moonraker` | Moonraker's JSON-RPC over a websocket, with reconnects |
| `crates/model` | the printer's state, built from Moonraker's status updates |
| `crates/app` | the `rivet` program: the engine that talks to Moonraker, settings, files, previews, Wi-Fi |
| `crates/fbdev-platform` | Slint on the bare Linux framebuffer and evdev: drawing, touch, backlight, screen sleep |
| `crates/ui-snapshot` | renders any screen to a PNG (and raw framebuffer images for the USB installer) |
| `crates/mock-moonraker` | a fake Moonraker with scenarios, for working without a printer |
| `ui/` | the screens, in Slint; `ui/kit` the shared controls; `ui/generated` made by `tools/gen-forms.py` |
| `i18n/ru` | Russian translation (gettext `.po`, compiled into the binary) |
| `assets/fonts` | Inter, cut down to Latin and Cyrillic by `tools/subset-fonts.sh` |
| `docs/design` | the design: prototype pages, design rules, where each value comes from |
| `tools/` | installers, release packaging, generators, printer helpers |

The engine runs on its own thread (tokio, single-threaded) and hands the UI finished values; the UI
thread only draws and reads touch. On the printer Rivet draws with Slint's software renderer into
memory and copies only the changed rectangle to the framebuffer, flipping between two pages.

**Touch.** Events from evdev (`ts_uinput` on Klipper Mod, or the raw panel through tslib's
`pointercal`) go through: the user's nine-point calibration; a short "landing" while the finger
settles; tap targeting (a tap that misses every button moves to the nearest one within 32 px);
a tap/drag decision (14 px, two samples); a press that stays on its button while nothing under it
scrolls. All of it is in `crates/fbdev-platform/src/{platform,touch_state,snap}.rs`, with tests on
recordings of real fingers on an AD5M.

**Screens follow the design.** Settings lists are generated from the prototype pages in
`docs/design/style` by `tools/gen-forms.py` (spec: `ui/spec/forms.toml`), icons by
`tools/gen-icons.py`. `tools/compare-prototype.py` compares rendered screens with the prototype
pixel by pixel. Values on screens come from Moonraker only ([data-sources](design/data-sources.md));
nothing is made up.

## Translations

Strings in `.slint` are `@tr("English")`. Add the Russian in `i18n/ru/LC_MESSAGES/rivet.po`;
`tools/extract-tr.py --check` lists what is missing.

## Making a release

A release is what printers update from, so the rules below are not optional.

1. On `main`, with CI green: set `version` in `Cargo.toml` (for example `0.1.1`), and add a section
   to `CHANGELOG.md` headed exactly `## v0.1.1-alpha` (the workflow uses it as the release notes).
2. Commit, then tag and push the tag:

   ```bash
   git tag v0.1.1-alpha
   git push origin v0.1.1-alpha
   ```

3. `.github/workflows/release.yml` builds the binary, runs `tools/package-release.sh` and publishes
   the release with:

   | Asset | For |
   |---|---|
   | `rivet-ad5m.zip` (first) | updates: Moonraker on Klipper Mod takes a release's first asset, Z-Mod the asset named in `release_info.json` |
   | `rivet-<tag>-usb.zip` | installing from a USB stick |
   | `rivet-<tag>-usb-remove.zip` | removing from a USB stick |
   | `rivet-<tag>-installer.tar.gz` | installing over ssh |

4. Check on a printer: Settings › Updates offers the new version, *Update the screen* installs it
   and the screen comes back.

Do not:

- rename `rivet-ad5m.zip` or upload anything before it: every installed Rivet looks for it;
- mark a release as a pre-release: Z-Mod (`channel: stable`) would not see it;
- delete a published release: Moonraker's roll-back downloads the previous one;
- tag a version that does not match `Cargo.toml` (the workflow refuses).

`tools/package-release.sh VERSION DIR` makes the same files locally, to test a USB stick before
tagging.

## Repository

Branches: `main` always builds and passes CI. Work goes in a branch and a pull request.
Commit messages in the Conventional Commits style (`feat(touch): …`, `fix(update): …`).
