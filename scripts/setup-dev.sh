#!/usr/bin/env bash
# Piperine development environment setup
# Installs system dependencies and configures environment variables.
#
# Usage:
#   bash scripts/setup-dev.sh
#
# After running, add the export lines to your shell profile (~/.bashrc / ~/.zshrc):
#   export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
#   export PATH="/usr/lib/llvm-18/bin:$PATH"

set -euo pipefail

LLVM_VERSION=18

# ── System packages ──────────────────────────────────────────────────────────

echo ">>> Installing system packages..."

sudo apt-get update -qq

# Core build tools
sudo apt-get install -y \
    build-essential \
    curl \
    git \
    pkg-config

# LLVM 18 + Clang 18 — required by piperine-openvaf (OpenVAF uses llvm-sys 181)
# NOTE: clang-18 must be first in PATH at build time so OpenVAF's osdi/build.rs
#       compiles stdlib.c with LLVM 18 bitcode (not LLVM 19 which is incompatible).
sudo apt-get install -y \
    llvm-${LLVM_VERSION} \
    llvm-${LLVM_VERSION}-dev \
    clang-${LLVM_VERSION} \
    libclang-${LLVM_VERSION}-dev \
    lld-${LLVM_VERSION} \
    libpolly-${LLVM_VERSION}-dev

# ngspice 46+ from trixie-backports — required for OSDI support (pre_osdi command).
# ngspice 44.x from trixie/main was compiled WITHOUT --enable-osdi.
# ngspice 46+ from trixie-backports includes --enable-osdi.
echo ">>> Installing ngspice 46 from trixie-backports (required for OSDI)..."
sudo apt-get install -y -t trixie-backports \
    libngspice0 \
    libngspice0-dev

# ── Rust toolchain ────────────────────────────────────────────────────────────

if ! command -v rustup &>/dev/null; then
    echo ">>> Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
else
    echo ">>> rustup found, updating..."
    rustup update stable
fi

# ── Environment variables ─────────────────────────────────────────────────────

LLVM_PREFIX="$(llvm-config-${LLVM_VERSION} --prefix)"

echo ""
echo ">>> LLVM ${LLVM_VERSION} prefix: ${LLVM_PREFIX}"
echo ""
echo "Add the following to your shell profile (~/.bashrc or ~/.zshrc):"
echo ""
echo "    export LLVM_SYS_181_PREFIX=${LLVM_PREFIX}"
echo "    export PATH=\"/usr/lib/llvm-${LLVM_VERSION}/bin:\$PATH\""
echo ""

# Export for the current shell session.
export LLVM_SYS_181_PREFIX="${LLVM_PREFIX}"
export PATH="/usr/lib/llvm-${LLVM_VERSION}/bin:${PATH}"
echo "LLVM_SYS_181_PREFIX and PATH exported for this session."

# ── Verify ────────────────────────────────────────────────────────────────────

echo ""
echo ">>> Verifying build (cargo check -p piperine-openvaf)..."
cargo check -p piperine-openvaf

echo ""
echo ">>> Setup complete."
echo "    Build:     LLVM_SYS_181_PREFIX=\$(llvm-config-18 --prefix) cargo build"
echo "    Run:       cargo run -- examples/diode_op.ppr"
