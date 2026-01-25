#!/bin/bash

# Wrapper script for natgeo-wallpapers CLI
# Downloads today's photo and sets wallpaper(s)
#
# Usage: ./set_wallpaper.sh [mode]
#   mode: monitors (default), virtual-desktops, or both

set -e

# Get the directory where this script is located
SCRIPT_DIR=$(dirname "$(readlink -f "$0")")

# Path to the Rust binary (will be updated by install script)
RUST_BINARY="${SCRIPT_DIR}/target/release/natgeo-wallpapers"

# Parse mode argument
MODE="${1:-monitors}"

# Check if Rust binary exists
if [ ! -f "$RUST_BINARY" ]; then
    echo "ERROR: Rust binary not found at $RUST_BINARY"
    echo "Please build the project first: cd $SCRIPT_DIR && cargo build --release"
    exit 1
fi

# Download today's photo
"$RUST_BINARY" download

# Set wallpaper(s) with the specified mode
"$RUST_BINARY" set "$MODE"
