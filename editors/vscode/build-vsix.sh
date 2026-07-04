#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# build-vsix.sh — Build the Piperine VS Code extension with embedded binaries
#
# Usage:
#   ./editors/vscode/build-vsix.sh            # release build
#   ./editors/vscode/build-vsix.sh --debug    # debug build (faster, larger)
#
# The script:
#   1. Builds piperine-lang-server and piperine CLI (release by default)
#   2. Copies the binaries into editors/vscode/bin/
#   3. Runs npm install + vsce package
#   4. Outputs piperine-<version>.vsix
# ---------------------------------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
VSCODE_DIR="$SCRIPT_DIR"

PROFILE="release"
CARGO_FLAG="--release"
if [[ "${1:-}" == "--debug" ]]; then
    PROFILE="debug"
    CARGO_FLAG=""
fi

echo "==> Building piperine binaries ($PROFILE)..."
cargo build $CARGO_FLAG \
    --manifest-path "$PROJECT_ROOT/Cargo.toml" \
    -p piperine-lang-server \
    -p piperine

TARGET_DIR="$PROJECT_ROOT/target/$PROFILE"
BIN_DIR="$VSCODE_DIR/bin"

mkdir -p "$BIN_DIR"

echo "==> Copying binaries to $BIN_DIR..."
cp "$TARGET_DIR/piperine-lang-server" "$BIN_DIR/"
cp "$TARGET_DIR/piperine"             "$BIN_DIR/"

# Make sure they're executable
chmod +x "$BIN_DIR/piperine-lang-server" "$BIN_DIR/piperine"

echo "==> Binaries:"
ls -lh "$BIN_DIR/"

echo "==> Installing npm dependencies..."
cd "$VSCODE_DIR"
npm install --ignore-scripts

echo "==> Packaging VSIX..."
npx vsce package --allow-missing-repository --no-update-package-json

VSIX_FILE=$(ls -1t "$VSCODE_DIR"/*.vsix 2>/dev/null | head -1)
if [[ -n "$VSIX_FILE" ]]; then
    echo ""
    echo "==> Done! Extension packaged:"
    echo "    $VSIX_FILE"
    echo ""
    echo "    Install with:  code --install-extension $VSIX_FILE"
else
    echo "ERROR: No .vsix file found after packaging."
    exit 1
fi
