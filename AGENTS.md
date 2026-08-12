# Prelude — AGENTS.md

## Quick start

```bash
# dev shell (includes Rust + gtk4 + libadwaita + alsa-lib)
nix develop

# build & run
nix build && nix run .
```

## Architecture

- Single crate at repo root (no workspace).
- `src/main.rs` creates an `adw::Application` with app-id `top.vikasmi.Prelude`, runs `application::PreludeApplication`.
- `src/application.rs` owns all GTK widget wiring — reads `ui/window.blp` (compiled to GtkBuilder XML by `build.rs` into `OUT_DIR`); runs a `glib::timeout_add_local` tick loop every 20 ms.
- `src/engine.rs` parses MIDI via `midly`, sends events via `midir`; handles play/pause/stop/seek/port management.
- `ui/window.blp` (Blueprint) is the only UI definition file. Change it → `build.rs` recompiles it on the next `cargo build`.

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
| `nix build . --option builders ''` | the **only** test gate — builds locally, runs `cargo test` (checkPhase) then `cargo clippy -- -D warnings` (postCheck). Never verify with bare `cargo ...` outside `nix develop`; `cargo` usage is limited to `cargo generate-lockfile` |
| `nix flake check` | verifies flake evaluation + formatting |
| `nix fmt` | format all files (Nix + Rust) via treefmt-nix |
| `nix develop` | dev shell with `cargo build` / `cargo clippy` |

There are no tests — no test directory, no test dependencies. Do not add testing infrastructure unless explicitly asked.

## Lint

`unwrap()` and `expect()` are **compile errors** (`unwrap_used`/`expect_used = deny` in `Cargo.toml`). All clippy warnings are fatal in postCheck (the `nix build` gate).

## Nix

- Flake inputs: `nixpkgs/nixpkgs-unstable`, `treefmt-nix`.
- Rust toolchain from nixpkgs (`rustPlatform`), supported system `x86_64-linux` only.
- `package.nix` reads `pname`/`version` from `Cargo.toml` via `lib.importTOML` — single source of truth, never hardcode them.
- `treefmt.nix` holds the formatter config; `flake.nix` only evaluates it.
- `devShells.default` uses `inputsFrom` the package — dependency lists are not duplicated.
- `nix run .` works via `meta.mainProgram`; there is no `apps` output.
- Both `Cargo.lock` and `flake.lock` are committed.

## Constraints

- **UI template is compiled in**: `ui/window.blp` → `build.rs` → GtkBuilder XML embedded via `include_str`; the generated `.ui` is never committed.
- **`build.rs` exists only to invoke `blueprint-compiler`** (a deliberate exception to the general no-build.rs policy — Blueprint needs a compile step). `blueprint-compiler` must be in `nativeBuildInputs` (package) / devShell.
- **No CI** — no workflows in `.github/workflows/`.
- **App is GPL-3.0-only**; license must be preserved on reuse.
- Target environment: **Linux** with a running ALSA sequencer or hardware MIDI port.