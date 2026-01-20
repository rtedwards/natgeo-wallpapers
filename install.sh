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

# Copy wallpaper script
cp "$SCRIPT_DIR/set_wallpaper.sh" "$USER_BIN/natgeo-set-wallpaper"
chmod +x "$USER_BIN/natgeo-set-wallpaper"

# Update the script to use the installed binary
sed -i "s|rust_binary=.*|rust_binary=\"$USER_BIN/$PROJECT_NAME\"|" "$USER_BIN/natgeo-set-wallpaper"
sed -i "s|photo_base_dir=.*|photo_base_dir=\"$PHOTO_DIR\"|" "$USER_BIN/natgeo-set-wallpaper"
sed -i "s|log_file=.*|log_file=\"$HOME/.local/share/$PROJECT_NAME/wallpaper.log\"|" "$USER_BIN/natgeo-set-wallpaper"

# Create log directory
mkdir -p "$HOME/.local/share/$PROJECT_NAME"
echo -e "${GREEN}✓${NC} Wallpaper script installed to $USER_BIN/natgeo-set-wallpaper"

# Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$USER_BIN:"* ]]; then
    echo -e "\n${YELLOW}Note: $USER_BIN is not in your PATH${NC}"
    echo "Add this to your ~/.bashrc or ~/.zshrc:"
    echo -e "${GREEN}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}"
fi

# Setup systemd timer
echo -e "\n${YELLOW}Setting up systemd timer...${NC}"

if ! command -v systemctl &> /dev/null; then
    echo -e "${RED}ERROR: systemctl not found${NC}"
    echo "This installer requires systemd for automation"
    echo "You can manually set up automation after installation"
else
    echo "When would you like the wallpaper to update?"
    echo "  1) Daily at 2:00 AM (recommended)"
    echo "  2) Daily at 6:00 AM"
    echo "  3) Daily at 8:00 AM"
    echo "  4) Custom time"
    echo "  5) Skip automation setup"

    read -p "Enter choice [1-5]: " time_choice

    WALLPAPER_CMD="$USER_BIN/natgeo-set-wallpaper"
    SCHEDULE_TIME=""

    case $time_choice in
        1)
            SYSTEMD_TIME="02:00"
            SCHEDULE_TIME="2:00 AM"
            ;;
        2)
            SYSTEMD_TIME="06:00"
            SCHEDULE_TIME="6:00 AM"
            ;;
        3)
            SYSTEMD_TIME="08:00"
            SCHEDULE_TIME="8:00 AM"
            ;;
        4)
            while true; do
                read -p "Enter time (HH:MM format, e.g., 22:45): " custom_time
                if [[ $custom_time =~ ^([0-1][0-9]|2[0-3]):([0-5][0-9])$ ]]; then
                    SYSTEMD_TIME="$custom_time"
                    SCHEDULE_TIME="$custom_time"
                    break
                else
                    echo -e "${RED}Invalid format. Please use HH:MM (00:00-23:59)${NC}"
                fi
            done
            ;;
        5)
            echo -e "${YELLOW}Skipping automation setup${NC}"
            ;;
        *)
            echo -e "${RED}Invalid choice, skipping automation setup${NC}"
            ;;
    esac

    if [ -n "$SCHEDULE_TIME" ]; then
        # Create systemd service directory
        mkdir -p "$HOME/.config/systemd/user"

        # Create systemd service
        cat > "$HOME/.config/systemd/user/natgeo-wallpaper.service" << EOF
[Unit]
Description=Download and set National Geographic Photo of the Day as wallpaper
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
ExecStart=$WALLPAPER_CMD
EOF

        # Create systemd timer
        cat > "$HOME/.config/systemd/user/natgeo-wallpaper.timer" << EOF
[Unit]
Description=Daily National Geographic Photo of the Day wallpaper update

[Timer]
OnCalendar=*-*-* $SYSTEMD_TIME:00
Persistent=true

[Install]
WantedBy=timers.target
EOF

        # Reload systemd, enable and restart the timer
        systemctl --user daemon-reload
        systemctl --user enable natgeo-wallpaper.timer 2>/dev/null
        systemctl --user restart natgeo-wallpaper.timer 2>/dev/null || systemctl --user start natgeo-wallpaper.timer

        echo -e "${GREEN}✓${NC} Systemd timer created and enabled: $SCHEDULE_TIME daily"
    fi
fi

# Download today's photo and set as wallpaper
echo -e "\n${YELLOW}Downloading today's photo and setting as wallpaper...${NC}"
if "$USER_BIN/natgeo-set-wallpaper" >> "$HOME/.local/share/$PROJECT_NAME/install.log" 2>&1; then
    echo -e "${GREEN}✓${NC} Photo downloaded and wallpaper set successfully"
else
    echo -e "${YELLOW}Warning: Could not download/set wallpaper (check logs)${NC}"
    echo -e "You can manually run: ${GREEN}natgeo-set-wallpaper${NC}"
fi

# Summary
echo -e "\n${GREEN}=== Installation Complete ===${NC}\n"
echo "Commands available:"
echo -e "  ${GREEN}$PROJECT_NAME${NC}           - Download today's photo"
echo -e "  ${GREEN}natgeo-set-wallpaper${NC}    - Download photo and set as wallpaper"
echo ""
echo "Photo storage:"
echo -e "  ${GREEN}$PHOTO_DIR${NC}"
echo ""
echo "Wallpaper picker:"
echo -e "  ${GREEN}$WALLPAPER_DIR${NC}"
echo ""
echo "Log file:"
echo -e "  ${GREEN}$HOME/.local/share/$PROJECT_NAME/wallpaper.log${NC}"
echo ""

if [ -n "$SCHEDULE_TIME" ]; then
    echo "Automation (systemd timer):"
    echo -e "  ${GREEN}$SCHEDULE_TIME daily${NC}"
    echo ""
    echo "To check status:"
    echo -e "  ${GREEN}systemctl --user status natgeo-wallpaper.timer${NC}"
    echo ""
    echo "To view timer logs:"
    echo -e "  ${GREEN}journalctl --user -u natgeo-wallpaper.service${NC}"
    echo ""
fi

echo "To test now, run:"
echo -e "  ${GREEN}natgeo-set-wallpaper${NC}"
echo ""
