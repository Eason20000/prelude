# Prelude — AGENTS.md

## Quick start

```bash
# dev shell (includes Rust + gtk4 + libadwaita + alsa-lib)
nix develop

# build & run
cargo build && cargo run

# or run directly via flake
nix run .
```

## Architecture

- Single crate at repo root (no workspace).
- `src/main.rs` creates an `adw::Application` with app-id `top.vikasmi.Prelude`, runs `application::PreludeApplication`.
- `src/application.rs` owns all GTK widget wiring — reads `ui/window.ui` via `include_str!("../ui/window.ui")` at compile time; runs a `glib::timeout_add_local` tick loop every 20 ms.
- `src/engine.rs` parses MIDI via `midly`, sends events via `midir`; handles play/pause/stop/seek/port management.
- `ui/window.ui` is the only UI definition file. Change it → rebuild required.

## Dependencies (non-obvious)

| Dep | Version | Notes |
|---|---|---|
| `gtk4` | `=0.11.3` feat `v4_14` | exact pin |
| `libadwaita` | `=0.9.1` feat `v1_8` | exact pin |
| `midly` | `0.5` | MIDI file parser |
| `midir` | `0.11` | MIDI output; requires `alsa-lib` at runtime on Linux |

## Developer commands

| Command | Notes |
|---|---|
| `cargo build` | standard build |
| `cargo clippy` | lint (available inside `nix develop`) |
| `cargo fmt` | format (available inside `nix develop`) |
| `nix fmt` | format all files (Nix + Rust) via treefmt-nix |
| `nix flake check` | verifies flake evaluation + formatting |

There are no tests — no test directory, no test dependencies. Do not add testing infrastructure unless explicitly asked.

## Nix

- Flake inputs: `nixpkgs/nixos-unstable`, `fenix` (follows nixpkgs), `treefmt-nix`.
- Rust toolchain from fenix (stable), not from nixpkgs.
- `buildInputs`: `gtk4`, `libadwaita`, `alsa-lib`.
- `nativeBuildInputs`: `pkg-config`, `wrapGAppsHook4`, `autoPatchelfHook`.
- Both `Cargo.lock` and `flake.lock` are committed.
- Supported systems: `x86_64-linux`, `aarch64-linux`.

## Constraints

- **UI template is compiled in**: edit `ui/window.ui` → rebuild.
- **No `build.rs`**, no codegen, no migrations.
- **No CI** — no workflows in `.github/workflows/`.
- **App is GPL-3.0-only**; license must be preserved on reuse.
- Target environment: **Linux** with a running ALSA sequencer or hardware MIDI port.
