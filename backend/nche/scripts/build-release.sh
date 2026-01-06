#!/usr/bin/env bash
#
# NCHE Release Build Script
#
# Builds optimized release binaries for multiple platforms.
#
# Usage:
#   ./scripts/build-release.sh              # Build for current platform
#   ./scripts/build-release.sh linux-x64    # Cross-compile for Linux x86_64
#   ./scripts/build-release.sh linux-arm64  # Cross-compile for Linux ARM64
#   ./scripts/build-release.sh all          # Build all platforms
#
# Prerequisites:
#   - Rust toolchain (rustup)
#   - cross (for cross-compilation): cargo install cross
#   - Docker (required by cross for cross-compilation)
#
# The script will:
#   1. Build the frontend (if not already built)
#   2. Prepare sqlx for offline mode (if needed)
#   3. Build the release binary
#   4. Output binary location and size

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
FRONTEND_DIR="$(dirname "$(dirname "$PROJECT_DIR")")/frontend"
TARGET="${1:-native}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() {
    echo -e "${GREEN}[BUILD]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Check if cross is installed for cross-compilation
check_cross() {
    if ! command -v cross &> /dev/null; then
        error "cross is not installed. Install with: cargo install cross"
    fi
    if ! command -v docker &> /dev/null; then
        error "Docker is required for cross-compilation but not found"
    fi
}

# Build frontend if needed
build_frontend() {
    if [ -d "$FRONTEND_DIR/out" ]; then
        log "Frontend already built (frontend/out exists)"
    else
        log "Building frontend..."
        cd "$FRONTEND_DIR"
        if command -v bun &> /dev/null; then
            bun install && bun run build
        elif command -v npm &> /dev/null; then
            npm install && npm run build
        else
            error "Neither bun nor npm found. Cannot build frontend."
        fi
        cd "$PROJECT_DIR"
    fi
}

# Prepare sqlx for offline mode
prepare_sqlx() {
    if [ -f "$PROJECT_DIR/.sqlx/query-*.json" ] 2>/dev/null || [ -d "$PROJECT_DIR/.sqlx" ]; then
        log "SQLx offline data found"
    else
        warn "SQLx offline data not found. For cross-compilation, run:"
        warn "  cargo sqlx prepare"
        warn "This requires DATABASE_URL to be set to a live database."
    fi
}

# Build for native platform
build_native() {
    log "Building for native platform (release)..."
    cd "$PROJECT_DIR"
    cargo build --release

    local binary="$PROJECT_DIR/target/release/nche"
    if [ -f "$binary" ]; then
        local size=$(du -h "$binary" | cut -f1)
        log "Build complete: $binary ($size)"
    fi
}

# Cross-compile for Linux x86_64 musl
build_linux_x64() {
    check_cross
    log "Cross-compiling for x86_64-unknown-linux-musl..."
    cd "$PROJECT_DIR"

    # Add target if not present
    rustup target add x86_64-unknown-linux-musl 2>/dev/null || true

    cross build --release --target x86_64-unknown-linux-musl

    local binary="$PROJECT_DIR/target/x86_64-unknown-linux-musl/release/nche"
    if [ -f "$binary" ]; then
        local size=$(du -h "$binary" | cut -f1)
        log "Build complete: $binary ($size)"
        log "This is a fully static binary - no external dependencies"
    fi
}

# Cross-compile for Linux ARM64 musl
build_linux_arm64() {
    check_cross
    log "Cross-compiling for aarch64-unknown-linux-musl..."
    cd "$PROJECT_DIR"

    # Add target if not present
    rustup target add aarch64-unknown-linux-musl 2>/dev/null || true

    cross build --release --target aarch64-unknown-linux-musl

    local binary="$PROJECT_DIR/target/aarch64-unknown-linux-musl/release/nche"
    if [ -f "$binary" ]; then
        local size=$(du -h "$binary" | cut -f1)
        log "Build complete: $binary ($size)"
        log "This is a fully static binary - no external dependencies"
    fi
}

# Main
log "NCHE Release Build"
log "=================="

build_frontend
prepare_sqlx

case "$TARGET" in
    native)
        build_native
        ;;
    linux-x64|linux-amd64|x86_64)
        build_linux_x64
        ;;
    linux-arm64|linux-aarch64|aarch64)
        build_linux_arm64
        ;;
    all)
        build_native
        build_linux_x64
        build_linux_arm64
        ;;
    *)
        error "Unknown target: $TARGET"
        echo "Usage: $0 [native|linux-x64|linux-arm64|all]"
        exit 1
        ;;
esac

log "Done!"
