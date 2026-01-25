#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get the directory where this script is located
SCRIPT_DIR=$(dirname "$(readlink -f "$0")")
PROJECT_NAME="natgeo-wallpapers"

echo -e "${GREEN}=== National Geographic Wallpaper Installer ===${NC}\n"

# Check if already installed
if [ -f "$HOME/.local/bin/natgeo-wallpapers" ]; then
    echo -e "${YELLOW}Note: Installation detected. This will update/reinstall.${NC}\n"
fi

# Check if Rust/Cargo is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Cargo is not installed${NC}"
    echo "Please install Rust from https://rustup.rs/"
    exit 1
fi

echo -e "${GREEN}✓${NC} Cargo found"

# Build the release binary
echo -e "\n${YELLOW}Building release binary...${NC}"
cd "$SCRIPT_DIR"
cargo build --release

if [ ! -f "target/release/$PROJECT_NAME" ]; then
    echo -e "${RED}Error: Build failed${NC}"
    exit 1
fi

echo -e "${GREEN}✓${NC} Binary built successfully"

# Create directories
echo -e "\n${YELLOW}Creating directories...${NC}"

# Create photo storage directory
PHOTO_DIR="$HOME/Pictures/NationalGeographic"
mkdir -p "$PHOTO_DIR"
echo -e "${GREEN}✓${NC} Created $PHOTO_DIR"

# Create KDE wallpaper picker directory
WALLPAPER_DIR="$HOME/.local/share/wallpapers/NationalGeographic"
mkdir -p "$WALLPAPER_DIR"
echo -e "${GREEN}✓${NC} Created $WALLPAPER_DIR"

# Create symlink from wallpaper picker to photo directory
echo -e "\n${YELLOW}Creating symlink for wallpaper picker...${NC}"
if [ -L "$WALLPAPER_DIR/photos" ]; then
    rm "$WALLPAPER_DIR/photos"
fi
ln -s "$PHOTO_DIR" "$WALLPAPER_DIR/photos"
echo -e "${GREEN}✓${NC} Symlink created: $WALLPAPER_DIR/photos -> $PHOTO_DIR"

# Create metadata file for KDE wallpaper picker
cat > "$WALLPAPER_DIR/metadata.desktop" << EOF
[Desktop Entry]
Name=National Geographic Photo of the Day

X-KDE-PluginInfo-Name=NationalGeographic
X-KDE-PluginInfo-Author=National Geographic
X-KDE-PluginInfo-License=Various
EOF
echo -e "${GREEN}✓${NC} Created wallpaper picker metadata"

# Install binary to user bin directory
echo -e "\n${YELLOW}Installing binary...${NC}"
USER_BIN="$HOME/.local/bin"
mkdir -p "$USER_BIN"
cp "target/release/$PROJECT_NAME" "$USER_BIN/$PROJECT_NAME"
chmod +x "$USER_BIN/$PROJECT_NAME"
echo -e "${GREEN}✓${NC} Binary installed to $USER_BIN/$PROJECT_NAME"

# Create log directory
mkdir -p "$HOME/.local/share/$PROJECT_NAME"

# Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$USER_BIN:"* ]]; then
    echo -e "\n${YELLOW}Note: $USER_BIN is not in your PATH${NC}"
    echo "Add this to your ~/.bashrc or ~/.zshrc:"
    echo -e "${GREEN}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}"
fi

# Summary
echo -e "\n${GREEN}=== Installation Complete ===${NC}\n"
echo "Commands available:"
echo -e "  ${GREEN}$PROJECT_NAME download${NC}     - Download today's photo"
echo -e "  ${GREEN}$PROJECT_NAME set${NC}          - Set wallpaper from downloaded photos"
echo -e "  ${GREEN}$PROJECT_NAME install${NC}      - Set up systemd timer for daily updates"
echo ""
echo "Photo storage:"
echo -e "  ${GREEN}$PHOTO_DIR${NC}"
echo ""

echo -e "${YELLOW}To set up automatic daily updates, run:${NC}"
echo -e "  ${GREEN}$PROJECT_NAME install${NC}"
echo ""
