# Windman — Windsurf Manager (userland, standalone)

Windman is a tiny CLI to manage the **Windsurf** IDE on systems without native packages (e.g., Arch Linux).  
It installs **userland/standalone** (no root), keeps a versioned directory layout, and switches atomically via a `current` symlink and a shell shim.

> Status: MVP. **Stable** channel only.

## Highlights

- **Userland** installs by default under:
  - Prefix: `~/.local/opt/windsurf`
  - Shim: `~/.local/bin/windsurf` → launches `$prefix/current/Windsurf/bin/windsurf`
- **Versioned installs** (`1.12.x`…); atomic switch via `current` symlink
- **Update** from official endpoint (**Linux only**)
- **List / Status / Rollback / Use <version> / Uninstall**
- **Keep policy**: keep N newest versions; protects current and *previous-current* for safe rollback
- **Desktop integration** (optional; enabled by default)
- Config file in `~/.config/windman/windman.toml`

## Why userland / standalone?

- Decoupled from distro package managers (fits Arch/AUR-less setups)
- No root needed; no `/opt` layout required (but supported if you change the prefix)
- Easy rollback/switch between versions

## Install (from source)

```bash
git clone https://github.com/<you>/Windman
cd Windman
cargo build --release
# Add to PATH or copy:
install -Dm755 target/release/windman ~/.local/bin/windman
```

## Quick start

```bash
# Initialize config with sensible defaults
windman config init

# Show effective config (paths resolved)
windman config show

# Install/update to the latest stable (Linux)
windman update

# Status / list versions
windman status
windman list

# Switch manually
windman use 1.12.11

# Rollback to the previous current
windman rollback

# Uninstall (keeps user data)
windman uninstall
```

## Configuration

`~/.config/windman/windman.toml` (default):

```toml
[install]
prefix_dir = "~/.local/opt/windsurf"
bin_dir = "~/.local/bin"
channel = "stable"
keep = 2
desktop_integration = true

[network]
proxy_enabled = false  # reserved for future proxy support
```

You can **override per-run**:
```bash
windman --prefix ~/Dev/windsurf --bin-dir ~/bin update
```

## Commands

- `update` — fetch latest stable (Linux) and install  
- `install --tar <FILE>` — install from a local tarball (useful for offline/test)  
- `list` — list installed versions; mark current  
- `status` — show paths and current version  
- `use <version>` — switch to a specific installed version  
- `rollback` — switch back to previous current  
- `uninstall` — remove installs and shim (optionally desktop files)  
- `where` — print paths  
- `config init/show` — manage config  

## Keep policy & safety

- `install.keep = N` keeps the **N newest** versions  
- Windman **always preserves**:  
  - the **current** version after the update  
  - the **previous-current** (the one that was active before the update)  
- This guarantees a safe one-step rollback after every update.  

## Targets

- **Linux x86_64**: `x86_64-unknown-linux-gnu`  
- **Linux AArch64**: `not supported because tarball isn't released by codeium yet`  
- macOS is **out of scope** for Windman (Windsurf provides its own macOS installer).

## Development

```bash
# Format & lint
cargo fmt
cargo clippy -- -D warnings

# Test
cargo test

# Taskfile (same locally and in CI)
task build
task test
task ci

# Package (host target by default; or set TARGET explicitly)
task dist:linux
task dist:linux TARGET=x86_64-unknown-linux-gnu
```

## License

MIT. © Ange Cesari.
