# NeoShell - Project Guide

## Project Overview
Cross-platform SSH/server management tool (like FinalShell) built with Tauri v2 + React + TypeScript.
Targets: macOS, Windows, Linux.

## Tech Stack
- **Frontend**: React 19 + TypeScript + Vite 7 + TailwindCSS v4
- **Routing**: react-router-dom v7
- **State**: Zustand v5
- **Terminal**: @xterm/xterm v6 + addons (fit, web-links, webgl)
- **Backend**: Rust / Tauri v2
- **SSH**: Rust `ssh2` crate (v0.9)
- **Async**: tokio (full features)
- **Package manager**: pnpm

## Critical Build Notes
The `/opt/bin/cc` linker is broken. ALL cargo/Rust commands MUST use:
```
RUSTFLAGS="-C linker=/usr/bin/cc" CC=/usr/bin/cc CXX=/usr/bin/c++
```

## Key Commands
```bash
# Frontend-only build (safe, no Rust)
pnpm build

# Dev mode (starts Vite dev server only)
pnpm dev

# Full Tauri dev (needs RUSTFLAGS above)
RUSTFLAGS="-C linker=/usr/bin/cc" CC=/usr/bin/cc CXX=/usr/bin/c++ pnpm tauri dev

# Full Tauri build
RUSTFLAGS="-C linker=/usr/bin/cc" CC=/usr/bin/cc CXX=/usr/bin/c++ pnpm tauri build
```

## Directory Structure
```
NeoShell/
  src/              # React + TypeScript frontend
  src-tauri/        # Rust backend (Tauri)
    src/lib.rs      # Main Tauri command handlers
    Cargo.toml      # Rust dependencies
    tauri.conf.json # App config
  docs/             # Project docs (gitignored)
    CHANGELOG.md
    tasks/
    summarize/
    mem/
  dist/             # Build output (gitignored)
```

## Architecture Notes
- SSH connections handled entirely in Rust backend via ssh2 crate
- Frontend communicates with Rust via Tauri `invoke()` commands
- Terminal UI uses xterm.js with WebGL renderer for performance
- Application state managed with Zustand stores
- Navigation with react-router-dom hash router (suitable for Tauri)

## Rust Backend (src-tauri/Cargo.toml) Dependencies
- `tauri-plugin-shell = "2"` - shell/process execution
- `tauri-plugin-opener = "2"` - file/URL opening  
- `ssh2 = "0.9"` - SSH client implementation
- `tokio = { full }` - async runtime
- `uuid = { v4 }` - connection ID generation
- `serde` / `serde_json` - serialization

## Notes
- `libssh2` must be available at build time for the ssh2 crate (brew install libssh2 on macOS)
- Tag-based releases trigger CI/CD (not main branch pushes)
- dev branch is local-only, never push to remote
