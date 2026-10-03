# Prelude — AGENTS.md

## Quick start

```bash
# dev shell (includes Rust + gtk4 + libadwaita + alsa-lib + blueprint-compiler)
nix develop

# build & run (the canonical build gate, see Developer commands)
nix build && nix run .
```

## Architecture

- Single crate at repo root (no workspace).
- `src/main.rs` creates an `adw::Application` with app-id `top.vikasmi.Prelude`,
  runs `application::PreludeApplication`.
- `src/application.rs` owns all GTK widget wiring — reads `ui/window.blp` and
  `ui/port_settings.blp` (both compiled to GtkBuilder XML by Meson into the
  installed `prelude.gresource` bundle, loaded at runtime via
  `Builder::from_resource`); `load_file` strips any extension via
  `Path::file_stem()` for `label_name` only (`engine.file_name` keeps the
  extension for the info sheet). Runs a `glib::timeout_add_local` tick loop
  every 20 ms. Scale seeks are **deferred to release** (`was_scale_active`
  flag): don't seek on every `change-value` while dragging — it spams
  `all_notes_off`. Port settings are an adaptive `Adw.PreferencesDialog`
  (`ui/port_settings.blp` → `Adw.ComboRow` + `StringList` injected via
  `PropertyExpression(StringObject:string)`) presented via
  `AdwDialogExt::present()`, floating on desktop / bottom-sheet on mobile.
- `src/engine.rs` parses MIDI via `midly`, sends events via `midir`; handles
  play/pause/stop/seek/port management. `play()` re-anchors
  `start = now - elapsed` for both Paused and Stopped (the old pause-duration
  compensation was deliberately removed — don't restore it). SMPTE/timecode
  files are rejected at load with an error. At load it also builds a measure map
  (tempo + time signature aware) and note intervals, exposed via `measures()` /
  `notes()` for the page-turn view.
- `src/page_view.rs` is a second custom `GtkWidget` subclass
  (`PreludePageTurnView`) — the P5 "kashiwade" page-turn visual. Dual-view
  layout in `view_root` (top to bottom): file title `Label label_name` (top-left
  `xalign 0.0` `ellipsize end` `title-1`, display stripped via `file_stem`),
  page-turn canvas (inside `Box page_view_placeholder[vexpand true]` which
  `append`s the widget, eats all extra space), density strip (inside
  `Box density_view_placeholder[vexpand false]` which `append`s the widget,
  natural `48` height, full width), control bar (`Adw.Clamp`, `valign: end`).
  Renders via `WidgetImpl::snapshot` (GPU render nodes, `append_color` only — no
  Cairo).
  - Driven by its **own `gtk::WidgetExt::add_tick_callback`** frame loop
    (display-refresh synchronized), independent of the 20 ms timeout loop.
  - Page-turn transition: on a forward page change the old page's notes are
    cached; after `SHRINK_INTERVAL_MS` a persistent `adw::SpringAnimation` (0→1,
    `SHRINK_*` params) drives the left-to-right shrink via
    `CallbackAnimationTarget`. Note growth is a **per-note fire-and-forget**
    `adw::SpringAnimation` (own `GROWTH_*` params) writing the note's `scale`
    cell (`Rc<Cell<f64>>`, fresh per cache rebuild); stale springs only touch
    orphaned cells. Growth `mass` is scaled by the note's visible length in
    quarter-note units (`Measure.quarter`, tempo-aware — one quarter note = 1×),
    so longer notes grow in more slowly. Note colors keep the accent hue with a
    deterministic per-channel lightness/saturation deviation
    (`TRACK_*_DEVIATION`, `track_color()` in HSL space). Both springs use a
    tight `*_EPSILON` so bars settle essentially exactly at full width. **No
    bezier easing anywhere** — spring physics only. All tunables are
    compile-time `const`s at the top of the file, never runtime settings.
    (`kashiwade` is the original composer's name; constants use the
    GROWTH/SHRINK scheme.)
  - Seek/jump clears the previous-page cache (no transition). Same accent redraw
    wiring as `midi_view.rs` (accent notify handler held in `imp`).
- `src/midi_view.rs` is a custom `GtkWidget` subclass (`PreludeMidiDensityView`)
  rendered via `WidgetImpl::snapshot` (GtkSnapshot → GPU-accelerated render
  nodes, `CONTENT_HEIGHT` halved to `48` for the strip); drag-to-scrub via
  `GestureDrag`. Played bars use the system accent color
  (`adw::StyleManager::accent_color_rgba`, non-deprecated), upcoming bars and
  the playhead use the widget foreground color. The placeholder `Box`
  `density_view_placeholder[vexpand false]` owns the outer `vexpand`, the widget
  itself is appended via `append` (no `remove`/`prepend` dance).
  - Drag is **content-grab** (drag right = rewind) — an intentional
    record-player model, not a bug; don't "fix" the sign.
  - GTK never auto-redraws this widget on accent changes (accent is not part of
    its CSS): `new()` subscribes to `connect_accent_color_rgba_notify` →
    `queue_draw`, with the handler id held in `imp` — keep that wiring.
  - Subclass traps if reworking: `glib::wrapper!` must declare
    `@implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget`, and
    `WidgetImpl::measure` returns
    `(min, natural, min_baseline, natural_baseline)`.
- `ui/window.blp` and `ui/port_settings.blp` (Blueprint) are the two UI
  definition files — `view_root` now declares `label_name` +
  `page/density_placeholder` parents via `append`, not `remove`/`prepend`.
  Change either → Meson recompiles on the next build (root `meson.build` →
  build-root `*.ui` → `src/prelude.gresource.xml` bundle in `pkgdatadir`, loaded
  at runtime; there is no `build.rs`).
- Meson is the primary build system, Nix only wraps it: root `meson.build`
  compiles each `.blp` to the build root (one explicit single-output target per
  file, no `batch-compile`), `src/meson.build` bundles them via
  `gnome.compile_resources` and invokes Cargo with `CARGO_TARGET_DIR` / `APP_ID`
  env, `data/meson.build` installs the desktop file, GSettings schema and icons.
  `src/config.rs` reads `APP_ID` via `option_env!()` with a plain-cargo fallback
  — never generate it, never `configure_file`+`cp` it. `gresource_path()`
  locates the bundle relative to the executable (`$PRELUDE_DATADIR` override,
  `../share/prelude`, `../Resources`, exe dir) with the baked `PKGDATADIR` as
  fallback, so portable trees run without installing to the build prefix.

## Dependencies (non-obvious)

| Dep           | Version                | Notes                                                |
| ------------- | ---------------------- | ---------------------------------------------------- |
| `gtk4`        | `=0.11.3` feat `v4_14` | exact pin                                            |
| `libadwaita`  | `=0.9.1` feat `v1_8`   | exact pin                                            |
| `graphene-rs` | `0.22`                 | `graphene::Rect` for `Snapshot::append_color`        |
| `midly`       | `0.5`                  | MIDI file parser                                     |
| `midir`       | `0.11`                 | MIDI output; requires `alsa-lib` at runtime on Linux |

## Developer commands

| Command                                             | Notes                                                                                                                                                                                                                                                                                         |
| --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `nix build`                                         | the **only** test gate — drives Meson (`meson setup`/`compile`/`install`); checkPhase still runs `cargo test` then `cargo clippy --profile release --offline -- -D warnings`. Never verify with bare `cargo ...` outside `nix develop`; `cargo` usage is limited to `cargo generate-lockfile` |
| `nix flake check`                                   | verifies flake evaluation + formatting                                                                                                                                                                                                                                                        |
| `nix fmt`                                           | format all tracked sources via treefmt-nix (Nix/Rust/TOML/Markdown/Blueprint/Meson/YAML/XML+SVG/CSS; `*.lock` and LICENSE excluded; yamlfmt folds the MSYS2 package list by design)                                                                                                           |
| `nix develop`                                       | dev shell with `cargo build` / `cargo clippy` / `cargo generate-lockfile`, plus `meson`/`blueprint-compiler` for setup+install to a prefix; plain `cargo build` still compiles but the binary needs installed resources to open a window                                                      |
| `meson setup builddir && meson compile -C builddir` | non-Nix builds (MSYS2 MinGW / native Linux): needs `cargo`, `blueprint-compiler`, gtk4 + libadwaita + alsa-lib visible; install with `meson install -C builddir`                                                                                                                              |

There are no tests — no test directory, no test dependencies. Do not add testing
infrastructure unless explicitly asked.

Run `nix fmt` before every commit.

## Lint

`unwrap()` and `expect()` are **compile errors**
(`unwrap_used`/`expect_used = deny` in `Cargo.toml`). All clippy warnings are
fatal in postCheck (the `nix build` gate).

GTK closures use `glib::clone!` with `#[strong]` / `#[weak]` attribute syntax
(glib 0.22 proc macro). The old `@strong x =>` syntax no longer exists — don't
reintroduce it. Weak captures auto-upgrade inside the closure; the handler id
must be held (in `imp`) or the connection is dropped.

## Nix

- Flake inputs: `nixpkgs/nixpkgs-unstable`, `treefmt-nix`.
- Rust toolchain from nixpkgs (`rustPlatform`), supported system `x86_64-linux`
  only.
- `package.nix` is `stdenv.mkDerivation` driving Meson (same shape as nixpkgs
  `fractal`): `cargoDeps = rustPlatform.fetchCargoVendor` + `cargoSetupHook` for
  offline vendoring, `mesonBuildType = "release"`. It reads `pname`/`version`
  from `Cargo.toml` via `lib.importTOML` — single source of truth, never
  hardcode them.
- After changing Cargo dependencies, reset `cargoDeps.hash` to `""`, run
  `nix build`, and copy back the `got: sha256-…` value.
- `treefmt.nix` holds the formatter config; `flake.nix` only evaluates it.
- `devShells.default` uses `inputsFrom` the package — dependency lists are not
  duplicated.
- `src = self` is git-filtered: new or renamed source files are invisible to
  `nix build` until `git add`ed.
- After changing `Cargo.toml`, regenerate the lock with
  `nix develop -c cargo generate-lockfile`.
- `nix run .` works via `meta.mainProgram`; there is no `apps` output.
- Both `Cargo.lock` and `flake.lock` are committed.

## Deferred work

`TODOS.md` tracks all deferred and open items with full context (location,
issue, expected behavior, rationale). Read it before adding new visuals or
parser changes. `README.md` intentionally has no roadmap.

## Constraints

- **UI templates load at runtime, nothing is embedded**: `ui/*.blp` → Meson
  custom targets → GtkBuilder XML → `prelude.gresource` bundle installed to
  `pkgdatadir`; `main` registers it before activate and exits nonzero with a
  stderr message when absent. There is no `build.rs`; the generated `.ui` is
  never committed. `blueprint-compiler` must be in `nativeBuildInputs` (package)
  / devShell.
- **CI builds all three platforms, runs no tests** —
  `.github/workflows/build.yml` (Windows MSYS2-UCRT64 / Ubuntu 26.04 / macOS 14)
  compiles each platform via Meson with `--buildtype=release` and uploads a
  runnable artifact (Windows zip in prefix layout, Linux AppImage via
  `linuxdeploy --plugin gtk`, macOS `.app` in a DMG); `release.yml` publishes
  them on GitHub Release. There is no CI smoke run — artifacts are verified by
  manual download-and-launch. The Nix gate owns tests and lint. Windows must
  stay on MSYS2-UCRT64 (MSVC/choco has no libadwaita or blueprint-compiler); its
  zip mirrors an install prefix (`bin/`/`lib/`/`share/`) with the Adwaita theme
  and the gdk-pixbuf loaders plus cache, which the relocatable MSYS2 libraries
  resolve relative to the exe. Linux pins `ubuntu-26.04` (24.04's libadwaita 1.5
  fails the `>=1.8` check); macOS ships unsigned (no signing or notarization —
  first launch needs right-click → Open), collecting pixbuf loaders from both
  the gdk-pixbuf and librsvg Homebrew prefixes (the SVG loader lives in
  librsvg's) into a cache template instantiated at launch.
  `packaging/macos/Info.plist` version keys are static — bump them with
  `Cargo.toml`. Theme icons ship inside all three artifacts (bundled Adwaita).
- **App is GPL-3.0-only**; license must be preserved on reuse.
- Target environment: **Linux** with a running ALSA sequencer or hardware MIDI
  port.
