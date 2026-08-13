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
- `src/main.rs` creates an `adw::Application` with app-id `top.vikasmi.Prelude`, runs `application::PreludeApplication`.
- `src/application.rs` owns all GTK widget wiring — reads `ui/window.blp` (compiled to GtkBuilder XML by `build.rs` into `OUT_DIR`); runs a `glib::timeout_add_local` tick loop every 20 ms. Scale seeks are **deferred to release** (`was_scale_active` flag): don't seek on every `change-value` while dragging — it spams `all_notes_off`.
- `src/engine.rs` parses MIDI via `midly`, sends events via `midir`; handles play/pause/stop/seek/port management. `play()` re-anchors `start = now - elapsed` for both Paused and Stopped (the old pause-duration compensation was deliberately removed — don't restore it). SMPTE/timecode files are rejected at load with an error.
- `src/midi_view.rs` is a custom `GtkWidget` subclass (`PreludeMidiDensityView`) rendered via `WidgetImpl::snapshot` (GtkSnapshot → GPU-accelerated render nodes); drag-to-scrub via `GestureDrag`. Played bars use the system accent color (`adw::StyleManager::accent_color_rgba`, non-deprecated), upcoming bars and the playhead use the widget foreground color.
  - Drag is **content-grab** (drag right = rewind) — an intentional record-player model, not a bug; don't "fix" the sign.
  - GTK never auto-redraws this widget on accent changes (accent is not part of its CSS): `new()` subscribes to `connect_accent_color_rgba_notify` → `queue_draw`, with the handler id held in `imp` — keep that wiring.
  - Subclass traps if reworking: `glib::wrapper!` must declare `@implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget`, and `WidgetImpl::measure` returns `(min, natural, min_baseline, natural_baseline)`.
- `ui/window.blp` (Blueprint) is the only UI definition file. Change it → `build.rs` recompiles it on the next `cargo build`.

## Dependencies (non-obvious)

| Dep | Version | Notes |
|---|---|---|
| `gtk4` | `=0.11.3` feat `v4_14` | exact pin |
| `libadwaita` | `=0.9.1` feat `v1_8` | exact pin |
| `graphene-rs` | `0.22` | `graphene::Rect` for `Snapshot::append_color` |
| `midly` | `0.5` | MIDI file parser |
| `midir` | `0.11` | MIDI output; requires `alsa-lib` at runtime on Linux |

## Developer commands

| Command | Notes |
|---|---|
| `nix build` | the **only** test gate — runs `cargo test` (checkPhase) then `cargo clippy -- -D warnings` (postCheck). Never verify with bare `cargo ...` outside `nix develop`; `cargo` usage is limited to `cargo generate-lockfile` |
| `nix flake check` | verifies flake evaluation + formatting |
| `nix fmt` | format all files (Nix + Rust + Blueprint `.blp`) via treefmt-nix |
| `nix develop` | dev shell with `cargo build` / `cargo clippy` / `cargo generate-lockfile` |

There are no tests — no test directory, no test dependencies. Do not add testing infrastructure unless explicitly asked.

Run `nix fmt` before every commit.

## Lint

`unwrap()` and `expect()` are **compile errors** (`unwrap_used`/`expect_used = deny` in `Cargo.toml`). All clippy warnings are fatal in postCheck (the `nix build` gate).

GTK closures use `glib::clone!` with `#[strong]` / `#[weak]` attribute syntax (glib 0.22 proc macro). The old `@strong x =>` syntax no longer exists — don't reintroduce it. Weak captures auto-upgrade inside the closure; the handler id must be held (in `imp`) or the connection is dropped.

## Nix

- Flake inputs: `nixpkgs/nixpkgs-unstable`, `treefmt-nix`.
- Rust toolchain from nixpkgs (`rustPlatform`), supported system `x86_64-linux` only.
- `package.nix` reads `pname`/`version` from `Cargo.toml` via `lib.importTOML` — single source of truth, never hardcode them.
- `treefmt.nix` holds the formatter config; `flake.nix` only evaluates it.
- `devShells.default` uses `inputsFrom` the package — dependency lists are not duplicated.
- `src = self` is git-filtered: new or renamed source files are invisible to `nix build` until `git add`ed.
- After changing `Cargo.toml`, regenerate the lock with `nix develop -c cargo generate-lockfile`.
- `nix run .` works via `meta.mainProgram`; there is no `apps` output.
- Both `Cargo.lock` and `flake.lock` are committed.

## Constraints

- **UI template is compiled in**: `ui/window.blp` → `build.rs` → GtkBuilder XML embedded via `include_str`; the generated `.ui` is never committed.
- **`build.rs` exists only to invoke `blueprint-compiler`** (a deliberate exception to the general no-build.rs policy — Blueprint needs a compile step). `blueprint-compiler` must be in `nativeBuildInputs` (package) / devShell.
- **No CI** — no workflows in `.github/workflows/`.
- **App is GPL-3.0-only**; license must be preserved on reuse.
- Target environment: **Linux** with a running ALSA sequencer or hardware MIDI port.