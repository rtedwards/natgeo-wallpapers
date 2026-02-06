# National Geographic Photo of the Day Downloader

A Rust-based tool that downloads the National Geographic Photo of the Day and sets it as your wallpaper, with support for multi-monitor setups and wallpaper rotation.

## Features

- Downloads the current National Geographic Photo of the Day
- Downloads entire monthly "Best of Photo of the Day" collections
- Sets wallpapers with multi-monitor and virtual desktop support
- Random wallpaper rotation from your photo collection
- Automatic scheduling with systemd timers (daily or interval-based)
- Organizes photos by date in `dd-mm-YYYY` format
- Supports KDE Plasma 6/5, GNOME, and X11 (feh)

## Prerequisites

- Rust and Cargo (install from [rustup.rs](https://rustup.rs/))
- Linux with systemd (for automatic scheduling)

## Installation

### Quick Install (Recommended)

```bash
git clone https://github.com/yourusername/natgeo-wallpapers.git
cd natgeo-wallpapers
./install.sh
```

The install script will:
- Build the release binary
- Install to `~/.local/bin/natgeo-wallpapers`
- Create photo directory at `~/Pictures/NationalGeographic/`
- Set up wallpaper picker integration for KDE

### Manual Build

```bash
cargo build --release
./target/release/natgeo-wallpapers --help
```

## Usage

### Commands Overview

```bash
natgeo-wallpapers                    # Download today's photo (default)
natgeo-wallpapers download           # Download today's photo
natgeo-wallpapers set [OPTIONS]      # Set wallpaper from downloaded photos
natgeo-wallpapers download-collection --url <URL>  # Download a monthly collection
natgeo-wallpapers install [OPTIONS]  # Set up automatic scheduling
```

### Download Today's Photo

```bash
natgeo-wallpapers download
```

### Download Monthly Collections

Download entire "Best of Photo of the Day" collections:

```bash
natgeo-wallpapers download-collection --url "https://www.nationalgeographic.com/photography/article/best-photos-october-2018"
```

Collections are saved to `~/Pictures/NationalGeographic/collections/<collection-name>/`

Browse available collections at: https://www.nationalgeographic.com/photography/topic/best-of-photo-of-the-day

### Set Wallpaper

```bash
# Set wallpaper from default directory (newest photos first)
natgeo-wallpapers set

# Set wallpaper from a specific directory
natgeo-wallpapers set --path ~/Pictures/NationalGeographic/collections

# Set a random wallpaper
natgeo-wallpapers set --random

# Set random wallpaper from a specific collection
natgeo-wallpapers set --random --path ~/Pictures/NationalGeographic/collections/best-photos-october-2018

# Also set lock screen (KDE Plasma only)
natgeo-wallpapers set --lock-screen
```

#### Multi-Monitor Modes

```bash
# Different wallpaper per monitor (default)
natgeo-wallpapers set --mode monitors

# Different wallpaper per virtual desktop
natgeo-wallpapers set --mode virtual-desktops

# Different wallpaper per monitor × virtual desktop
natgeo-wallpapers set --mode both
```

### Automatic Scheduling

Set up a systemd timer to automatically update wallpapers:

```bash
# Interactive setup (prompts for schedule)
natgeo-wallpapers install

# Daily at 2:00 AM (download new photo)
natgeo-wallpapers install --time 02:00

# Hourly random rotation from collections
natgeo-wallpapers install --time 1h --random --path ~/Pictures/NationalGeographic/collections

# Every 30 minutes
natgeo-wallpapers install --time 30m --random

# Uninstall the timer
natgeo-wallpapers install --uninstall
```

#### Schedule Options

When running `install` interactively, you'll see:

```
When would you like the wallpaper to update?
  1) Daily at 02:00 (recommended for daily photo)
  2) Every hour (good for random rotation)
  3) Every 30 minutes
  4) Custom time (HH:MM)
  5) Custom interval (e.g., 2h, 15m)
  6) Cancel
```

**Note:** Running `install` again will replace the previous timer configuration. You can only have one active timer at a time.

#### Timer Management

```bash
# Check timer status
systemctl --user status natgeo-wallpaper.timer

# View logs
journalctl --user -u natgeo-wallpaper.service

# Manually trigger
systemctl --user start natgeo-wallpaper.service

# Disable timer
systemctl --user disable natgeo-wallpaper.timer
systemctl --user stop natgeo-wallpaper.timer
```

## Directory Structure

```
~/Pictures/NationalGeographic/
├── 01-02-2026/                          # Daily photos by date
│   ├── Photo_Title.jpg
│   └── Photo_Title.log
├── 02-02-2026/
│   └── Another_Photo.jpg
└── collections/                         # Monthly collections
    ├── best-photos-october-2018/
    │   ├── 01-best-pod-october-18.jpg
    │   ├── 02-best-pod-october-18.jpg
    │   └── collection.log
    └── best-photos-september-2018/
        └── ...
```

## Supported Desktop Environments

| Environment | Tool Used | Multi-Monitor | Virtual Desktops |
|-------------|-----------|---------------|------------------|
| KDE Plasma 6 | qdbus6 | Yes | Yes |
| KDE Plasma 5 | qdbus | Yes | No |
| GNOME | gsettings | No | No |
| X11 | feh | No | No |

## Examples

### Build a Photo Collection and Rotate Hourly

```bash
# Download some collections
natgeo-wallpapers download-collection --url "https://www.nationalgeographic.com/photography/article/best-photos-october-2018"
natgeo-wallpapers download-collection --url "https://www.nationalgeographic.com/photography/article/best-photos-september-2018"

# Set up hourly random rotation
natgeo-wallpapers install --time 1h --random --path ~/Pictures/NationalGeographic/collections
```

### Daily Photo with Manual Rotation

```bash
# Set up daily download at 2am (no --random, uses newest photo)
natgeo-wallpapers install --time 02:00

# Manually set a random wallpaper anytime
natgeo-wallpapers set --random
```

### Set Specific Photo

```bash
natgeo-wallpapers set --path ~/Pictures/NationalGeographic/collections/best-photos-october-2018/01-best-pod-october-18.jpg
```

## Configuration

Photos are saved to `~/Pictures/NationalGeographic/` by default. To change this, edit the constants in `src/lib.rs`:

```rust
pub const PHOTO_SAVE_PATH: &str = "~/Pictures/NationalGeographic/";
pub const COLLECTION_SAVE_PATH: &str = "~/Pictures/NationalGeographic/collections/";
```

## Troubleshooting

### 403 Forbidden Error

The script uses browser-like headers to avoid blocking. If you still get 403 errors, the website structure may have changed.

### No Photos Found

Make sure you've downloaded some photos first:

```bash
natgeo-wallpapers download
# or
natgeo-wallpapers download-collection --url <collection-url>
```

### Check Logs

```bash
# View download logs
cat ~/Pictures/NationalGeographic/*/Photo_Title.log

# View wallpaper setting logs
cat ~/.local/share/natgeo-wallpapers/wallpaper.log

# View systemd service logs
journalctl --user -u natgeo-wallpaper.service --since today
```

## Development

### Running Tests

```bash
cargo test           # All tests
cargo clippy         # Linting
cargo fmt            # Format code
```

### Project Structure

```
src/
├── main.rs          # CLI and systemd setup
└── lib.rs           # Core logic (download, scraping, wallpaper)
tests/
└── integration.rs   # Integration tests
assets/
├── service.template # (legacy, now generated dynamically)
└── timer.template   # (legacy, now generated dynamically)
```

## Uninstall

```bash
# Uninstall timer
natgeo-wallpapers install --uninstall

# Remove binary
rm ~/.local/bin/natgeo-wallpapers

# Remove wallpaper picker integration
rm -rf ~/.local/share/wallpapers/NationalGeographic

# Optionally remove photos
rm -rf ~/Pictures/NationalGeographic

# Remove logs
rm -rf ~/.local/share/natgeo-wallpapers
```

## License

See the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Notes

- National Geographic's website structure may change over time, which could break the scraping logic
- The "Best of Photo of the Day" collections include photos from related months that appear on the page
- Only one systemd timer can be active at a time; running `install` again replaces the previous configuration
