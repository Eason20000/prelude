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
| `nix build` | build Linux binary |
| `nix build .#prelude-windows` | cross-compile Windows binary |

There are no tests — no test directory, no test dependencies. Do not add testing infrastructure unless explicitly asked.

## Nix

- Flake inputs: `nixpkgs/nixos-unstable`, `fenix` (follows nixpkgs), `treefmt-nix`.
- Rust toolchain from fenix (stable), not from nixpkgs.
- `buildInputs`: `gtk4`, `libadwaita`, `alsa-lib`.
- `nativeBuildInputs`: `pkg-config`, `wrapGAppsHook4`, `autoPatchelfHook`.
- Both `Cargo.lock` and `flake.lock` are committed.
- Supported systems: `x86_64-linux`, `aarch64-linux`.

## Windows cross-compilation

Cross-compiles Prelude to `x86_64-pc-windows-gnu` (MinGW target).

```bash
# build the Windows binary with bundled DLLs and runtime resources
nix build .#prelude-windows
```

### Architecture (see `nix/` directory)

| File | Role |
|---|---|
| `nix/windows-deps.nix` | Downloads 92 MSYS2 pre-built packages (GTK4, libadwaita, GStreamer, appstream + full transitive deps), rewrites `.pc` file prefixes to Nix store paths, removes static `.a` files (keeping only `.dll.a`), adds `libpthread.a → libmcfgthread.a` symlink for Rust's MinGW target compatibility. |
| `nix/windows-build.nix` | Cross-compilation derivation: fenix Rust toolchain with `x86_64-pc-windows-gnu` target, `pkgsCross.ucrt64` MinGW linker, vendored cargo deps, bundles `.exe` + 162 DLLs + GSettings schemas (compiled via `glib-compile-schemas`) + MIME database (via `update-mime-database`) + Adwaita icon theme + GTK4 settings. |

### Key design decisions

- **MSYS2 pre-built packages** (not source-build): GTK4/libadwaita can't cross-compile from source in nixpkgs. Instead, fetch UCRT64 binary packages from MSYS2 mirror.
- **`.dll.a` import libraries**: MSYS2 provides MinGW-compatible `.dll.a` files natively. Static `.a` are removed to force dynamic linking.
- **`.pc` prefix rewriting**: MSYS2 `.pc` files have `prefix=/ucrt64` which is rewritten to the Nix store path via `substituteInPlace`.
- **ALSA handling**: `alsa-lib` from nixpkgs is added to `nativeBuildInputs` so `alsa-sys` build script can probe on the host. The `midir` crate gates ALSA behind `cfg(target_os = "linux")` so it's not linked for Windows.
- **mcfgthreads**: nixpkgs' MinGW uses `mcfgthreads` instead of `winpthreads`. A `libpthread.a → libmcfgthread.a` symlink is created for Rust's MinGW target.
- **GStreamer + appstream**: Despite Prelude not needing video, the MSYS2-built `libgtk-4-1.dll` has hard runtime deps on GStreamer, and `libadwaita-1-0.dll` on appstream. These are included.
- **GDK-Pixbuf loader cache**: Linux `gdk-pixbuf-query-loaders` can't introspect Windows PE `.dll` files. GDK-Pixbuf on Windows auto-discovers loaders from the standard directory at runtime.

### Creating a test distribution

```bash
nix build .#prelude-windows
OUT=$(nix eval --impure --raw .#prelude-windows)
cp -r "$OUT"/* /tmp/prelude/
(cd /tmp && zip -r ~/prelude-windows-test.zip prelude/)
```

Output: ~53 MB zip containing `bin/` (exe + 162 DLLs), `lib/` (pixbuf loaders), `share/` (icons, schemas, mime), `etc/` (GTK4 settings).

## Constraints

- **UI template is compiled in**: edit `ui/window.ui` → rebuild.
- **No `build.rs`**, no codegen, no migrations.
- **No CI** — no workflows in `.github/workflows/`.
- **App is GPL-3.0-only**; license must be preserved on reuse.
- Target environment: **Linux** with a running ALSA sequencer or hardware MIDI port.
