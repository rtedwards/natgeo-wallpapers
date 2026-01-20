#!/bin/bash

# Script to download National Geographic Photo of the Day using Rust binary
# and set it as the KDE/Plasma wallpaper

# Get the directory where this script is located
current_path=$(dirname "$(readlink -f "$0")")

# Path to the Rust binary (will be updated by install script)
rust_binary="${current_path}/target/release/natgeo-wallpapers"

# Directory where photos are saved (will be updated by install script)
photo_base_dir="$HOME/Pictures/NationalGeographic"
today_date=$(date '+%d-%m-%Y')
photo_dir="${photo_base_dir}/${today_date}"

# Log file (will be updated by install script)
log_file="$HOME/.local/share/natgeo-wallpapers/wallpaper.log"

# Create log directory if it doesn't exist
mkdir -p "$(dirname "$log_file")"

# Function to log messages with timestamps
log_message() {
    echo "$(date '+%Y-%m-%d %H:%M:%S') - $1" | tee -a "$log_file"
}

log_message "=== Starting National Geographic wallpaper update ==="

# Check if Rust binary exists
if [ ! -f "$rust_binary" ]; then
    log_message "ERROR: Rust binary not found at $rust_binary"
    log_message "Please build the project first: cd $current_path && cargo build --release"
    exit 1
fi

# Run the Rust binary to download today's photo
log_message "Running Rust binary to download photo..."
if ! "$rust_binary" >> "$log_file" 2>&1; then
    log_message "ERROR: Rust binary failed to download photo"
    exit 2
fi

log_message "Photo download completed successfully"

# Find the downloaded photo (should be a .jpg, .png, or .gif file)
photo_file=$(find "$photo_dir" -maxdepth 1 -type f \( -name "*.jpg" -o -name "*.png" -o -name "*.gif" \) -print -quit)

if [ -z "$photo_file" ]; then
    log_message "ERROR: No photo file found in $photo_dir"
    exit 3
fi

log_message "Found photo: $photo_file"

# Convert to file:// URL format for KDE
# Escape single quotes for JavaScript context
wallpaper_url="file://${photo_file//\'/\\\'}"

# Set the wallpaper for KDE Plasma
log_message "Setting wallpaper to: $wallpaper_url"

# Detect desktop environment and set wallpaper accordingly
if command -v plasma-apply-wallpaperimage &> /dev/null && pgrep -x plasmashell > /dev/null; then
    # KDE Plasma 6 - Use plasma-apply-wallpaperimage (preferred method)
    log_message "Detected KDE Plasma 6, using plasma-apply-wallpaperimage..."
    plasma-apply-wallpaperimage "$photo_file" >> "$log_file" 2>&1

    if [ $? -eq 0 ]; then
        log_message "Wallpaper set successfully via plasma-apply-wallpaperimage"
    else
        log_message "WARNING: plasma-apply-wallpaperimage command failed"
    fi
elif command -v qdbus6 &> /dev/null && pgrep -x plasmashell > /dev/null; then
    # KDE Plasma 6 - Fallback to qdbus6
    log_message "Using qdbus6 fallback..."
    qdbus6 org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript "
        var allDesktops = desktops();
        for (i=0;i<allDesktops.length;i++) {
            d = allDesktops[i];
            d.wallpaperPlugin = 'org.kde.image';
            d.currentConfigGroup = Array('Wallpaper', 'org.kde.image', 'General');
            d.writeConfig('Image', '${wallpaper_url}');
        }
    " >> "$log_file" 2>&1

    if [ $? -eq 0 ]; then
        log_message "Wallpaper set successfully via qdbus6"
    else
        log_message "WARNING: qdbus6 command failed"
    fi
elif command -v qdbus &> /dev/null && pgrep -x plasmashell > /dev/null; then
    # KDE Plasma 5 - Use qdbus
    log_message "Detected KDE Plasma 5, using qdbus..."
    qdbus org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript "
        var allDesktops = desktops();
        for (i=0;i<allDesktops.length;i++) {
            d = allDesktops[i];
            d.wallpaperPlugin = 'org.kde.image';
            d.currentConfigGroup = Array('Wallpaper', 'org.kde.image', 'General');
            d.writeConfig('Image', '${wallpaper_url}');
        }
    " >> "$log_file" 2>&1

    if [ $? -eq 0 ]; then
        log_message "Wallpaper set successfully via qdbus"
    else
        log_message "WARNING: qdbus command failed"
    fi
elif command -v gsettings &> /dev/null; then
    # GNOME/Ubuntu - Use gsettings
    log_message "Using gsettings for GNOME/Ubuntu..."
    gsettings set org.gnome.desktop.background picture-uri "$wallpaper_url" >> "$log_file" 2>&1
    gsettings set org.gnome.desktop.background picture-uri-dark "$wallpaper_url" >> "$log_file" 2>&1
    log_message "Wallpaper set via gsettings"
elif command -v feh &> /dev/null; then
    # Generic X11 - Use feh
    log_message "Using feh for generic X11..."
    feh --bg-scale "$photo_file" >> "$log_file" 2>&1
    log_message "Wallpaper set via feh"
else
    log_message "WARNING: No supported wallpaper tool found (plasma-apply-wallpaperimage, qdbus6, qdbus, gsettings, or feh)"
    log_message "Photo downloaded but wallpaper not set automatically"
fi

log_message "=== Wallpaper update completed ==="

exit 0
