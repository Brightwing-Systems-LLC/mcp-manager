# CLAUDE.md — Instructions for AI agents working on this project

## Project Overview

Brightwing MCP Manager is a Tauri v2 desktop app (Rust backend + React/TypeScript frontend) that manages MCP server installations, authentication, and proxying across AI tools (Claude Desktop, Cursor, VS Code, Claude Code, Codex, etc.).

## Build & Test

```bash
# Frontend only
npm run build

# Rust check + tests
cd src-tauri && cargo check && cargo test

# Full app build (local, unsigned)
./build-app.sh

# Full app build with updater signing
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/brightwing.key)" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" npm run tauri build

# Install to /Applications (macOS)
rm -rf "/Applications/Brightwing MCP Manager.app"
cp -R "src-tauri/target/release/bundle/macos/Brightwing MCP Manager.app" /Applications/
```

**Note:** Sidecar binaries (brightwing-authd, brightwing-proxy, bw) must exist in `src-tauri/binaries/` with target-triple suffixes before `cargo check` or `tauri build` will work. `build-app.sh` handles this automatically. In CI, placeholders are created first.

## Release Process

To ship a new version:

1. **Bump version** in all four places:
   - `src-tauri/tauri.conf.json` → `"version"`
   - `src-tauri/Cargo.toml` → `version`
   - `package.json` → `"version"`
   - `src/components/Navigation.tsx` → `APP_VERSION`
   - `src/components/About.tsx` → version string

2. **Commit** the version bump

3. **Tag and push**:
   ```bash
   git tag v<VERSION>
   git push origin main --tags
   ```

4. The `v*` tag triggers `.github/workflows/build.yml` which:
   - Builds for macOS (arm64 + x64), Windows, Linux
   - Signs updater artifacts with Ed25519 key
   - Creates a **draft** GitHub Release with .dmg, .app.tar.gz, .sig, and latest.json

5. **Publish** the draft release on GitHub to make it live

6. Running app instances will detect the update via `latest.json` and show "Update to vX.Y.Z" in the sidebar

## CI

- **ci.yml**: Runs on push to main (not tags) and PRs. TypeScript check + Rust check + tests.
- **build.yml**: Runs on `v*` tags only. Full multi-platform build + release.

## Key Architecture

- **Tauri commands** are in `src-tauri/src/lib.rs`
- **Database** (SQLite): schema in `src-tauri/src/db/migrations.rs`, queries in `queries.rs`
- **Config reader/writer**: `src-tauri/src/config/reader.rs` and `writer.rs`
- **Daemon** (brightwing-authd): `src-tauri/src/bin/brightwing_authd.rs`
- **Proxy**: `src-tauri/src/bin/brightwing_proxy.rs`
- **CLI shim** (bw): `src-tauri/src/bin/bw.rs`
- **Frontend store**: `src/store.ts` (Zustand)
- **Frontend Tauri bindings**: `src/lib/tauri.ts`

## Conventions

- Hardcoded version strings exist in Navigation.tsx and About.tsx — always bump these with the version
- Claude Code uninstalls edit `~/.claude.json` directly (not via `claude mcp remove`) to avoid hangs
- Sidecar binaries use Tauri's `externalBin` and require `<name>-<target-triple>` naming
