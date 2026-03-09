#!/usr/bin/env bash
set -e

TARGET=$(rustc -vV | grep '^host:' | awk '{print $2}')
BINARIES_DIR="src-tauri/binaries"

echo "▸ Building sidecar binaries (release)..."
cd src-tauri
# Build just the binaries from the workspace, bypassing the lib build.rs
# by compiling them directly with rustc via cargo
cargo build --release -p proxy-common
cargo rustc --release --bin brightwing-authd -- 2>/dev/null || true
cargo rustc --release --bin brightwing-proxy -- 2>/dev/null || true
cargo rustc --release --bin bw -- 2>/dev/null || true
cd ..

# If the above didn't work due to build.rs, create placeholder files first
mkdir -p "$BINARIES_DIR"
for name in brightwing-authd brightwing-proxy bw; do
    dest="$BINARIES_DIR/${name}-${TARGET}"
    if [ ! -f "$dest" ]; then
        # Create a placeholder so build.rs doesn't fail
        touch "$dest"
        chmod +x "$dest"
    fi
done

# Now do the full cargo build which will succeed since placeholders exist
echo "▸ Building all release binaries..."
cd src-tauri
cargo build --release --bins
cd ..

echo "▸ Copying sidecars for target: $TARGET"
for name in brightwing-authd brightwing-proxy bw; do
    cp "src-tauri/target/release/$name" "$BINARIES_DIR/${name}-${TARGET}"
    chmod +x "$BINARIES_DIR/${name}-${TARGET}"
done

echo "▸ Building Tauri app bundle..."
npm run tauri build

echo ""
echo "════════════════════════════════════════════════"
echo "  Build complete!"
echo "════════════════════════════════════════════════"
echo ""
echo "App bundle:  src-tauri/target/release/bundle/macos/"
echo "DMG:         src-tauri/target/release/bundle/dmg/"
echo ""
ls -lh src-tauri/target/release/bundle/macos/*.app 2>/dev/null || true
ls -lh src-tauri/target/release/bundle/dmg/*.dmg 2>/dev/null || true
