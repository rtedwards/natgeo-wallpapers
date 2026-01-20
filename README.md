# National Geographic Photo of the Day Downloader

A Rust-based tool that downloads the National Geographic Photo of the Day and saves it to a dated directory.

## Features

- Downloads the current National Geographic Photo of the Day
- Automatically detects image format (jpg, png, gif)
- Organizes photos by date in `dd-mm-YYYY` format
- Mimics browser headers to bypass CloudFront protection
- Simple and lightweight

## Prerequisites

- Rust and Cargo (install from [rustup.rs](https://rustup.rs/))

## Installation

### Quick Install (Recommended)

Run the install script to automatically set everything up:

```bash
git clone https://github.com/yourusername/natgeo-wallpapers.git
cd natgeo-wallpapers
make install
```

The install script will:
- Build the release binary
- Install to `~/.local/bin/natgeo-wallpapers`
- Create photo directory at `~/Pictures/NationalGeographic/`
- Set up wallpaper picker integration for KDE
- Install `natgeo-set-wallpaper` command
- Optionally configure cron job (default: daily at 2:00 AM)

### Manual Installation

1. Clone this repository:
```bash
git clone https://github.com/yourusername/natgeo-wallpapers.git
cd natgeo-wallpapers
```

2. Build and install:
```bash
make install
```

## Usage

### Basic Usage

Run the program to download today's photo:

```bash
natgeo-wallpapers
```

Or during development:

```bash
cargo run
```

### Logging

The script automatically creates a log file (`<photo-title>.log`) in the same directory as each downloaded photo. The log includes:

- Download start time
- Image URL
- Success/failure status
- Error details (if any)
- Completion timestamp

Example log file location: `./natgeo-photos/20-01-2026/NationalGeographic_433254.log`

### Configuration

By default, photos are saved to `~/Pictures/NationalGeographic/<dd-mm-YYYY>/`. To change the save location, edit the `PHOTO_SAVE_PATH` constant in `src/lib.rs`:

```rust
const PHOTO_SAVE_PATH: &str = "~/Pictures/NationalGeographic/"; // Change this to your preferred path
```

Then rebuild and reinstall:
```bash
make install
```

### Wallpaper Picker Integration

The install script creates a symlink at `~/.local/share/wallpapers/NationalGeographic/` that points to your photo directory. This makes downloaded photos appear in KDE Plasma's wallpaper picker under "National Geographic Photo of the Day".
```

## Output Structure

Photos are organized as follows:

```
~/Pictures/NationalGeographic/
├── 20-01-2026/
│   ├── NationalGeographic_433254.jpg
│   └── NationalGeographic_433254.log
├── 21-01-2026/
│   ├── NationalGeographic_789012.jpg
│   └── NationalGeographic_789012.log
└── ...
```

## Automation

The install script sets up a systemd timer by default. 

### Systemd Timer

The installer creates two files:
- `~/.config/systemd/user/natgeo-wallpaper.service`
- `~/.config/systemd/user/natgeo-wallpaper.timer`

**Check timer status:**
```bash
systemctl --user status natgeo-wallpaper.timer
```

**View logs:**
```bash
journalctl --user -u natgeo-wallpaper.service
```

**Manually trigger:**
```bash
systemctl --user start natgeo-wallpaper.service
```

**Disable timer:**
```bash
systemctl --user disable natgeo-wallpaper.timer
systemctl --user stop natgeo-wallpaper.timer
```

**Change schedule:**
Edit the timer file:
```bash
nano ~/.config/systemd/user/natgeo-wallpaper.timer
```

Then reload:
```bash
systemctl --user daemon-reload
systemctl --user restart natgeo-wallpaper.timer
```

## Setting Wallpaper Automatically

The `set_wallpaper.sh` script downloads the photo using the Rust binary and sets it as your desktop wallpaper.

### Usage

```bash
./set_wallpaper.sh
```

### Supported Desktop Environments

- **KDE Plasma 6** - Uses `plasma-apply-wallpaperimage` (preferred) or `qdbus6`
- **KDE Plasma 5** - Uses `qdbus`
- **GNOME/Ubuntu** - Uses `gsettings` 
- **Generic X11** - Uses `feh`

### Logs

The script logs to `~/.local/share/natgeo-wallpapers/wallpaper.log`

View logs:
```bash
tail -f ~/.local/share/natgeo-wallpapers/wallpaper.log
```

## How It Works

1. The script fetches the National Geographic Photo of the Day webpage
2. It parses the HTML to extract the Open Graph (og:image) meta tag containing the photo URL
3. The photo title is extracted from the og:title meta tag
4. The image is downloaded with proper browser headers to avoid blocking
5. The file is saved with the sanitized title as the filename

## Troubleshooting

### 403 Forbidden Error

If you encounter a 403 error, the script may need updated User-Agent headers. The script already includes modern browser headers, but these can be updated in the `get_current_web_natgeo_gallery()` and `download_natgeo_photo_of_the_day()` functions.

### Empty or Invalid Image URL

If the HTML structure of the National Geographic website changes, the parsing logic may need to be updated. The script looks for `property="og:image"` meta tags in the HTML.

### Check Logs

Run the program with verbose output to see what's happening:

```bash
RUST_LOG=debug cargo run
```

## Project Structure

The project follows Rust conventions with separated unit and integration tests:

```
src/
├── main.rs          # Entry point (72 lines)
├── lib.rs           # Core logic + unit tests (343 lines)
tests/
└── integration.rs   # Integration tests (118 lines)
```

## Development

### Testing

The project includes comprehensive unit and integration tests to ensure reliability.

**Run all checks (recommended before committing):**
```bash
make check
```

This runs:
- Code formatting check
- Clippy linting
- All tests

### Test Coverage

**Unit Tests (8 tests in `src/lib.rs`):**
- ✅ File extension detection from MIME types (jpg, png, gif)
- ✅ Content-type parsing with parameters
- ✅ Log file creation and formatting with timestamps
- ✅ Title sanitization (special characters: /, :, |)
- ✅ Title length limiting (max 100 characters)
- ✅ Date format validation (dd-mm-YYYY)
- ✅ HTML parsing for og:image and og:title meta tags
- ✅ Mock image file download and save

**Integration Tests (4 tests in `tests/integration.rs`):**
- ✅ Real network image download (via httpbin)
- ✅ Log and photo file co-location verification
- ✅ Error log creation and messages
- ✅ Full workflow simulation (fetch → download → log)

### Running Tests

**Run all checks:**
```bash
make check      # Format check + linting + tests
```

**Run only tests:**
```bash
make test       # All tests
cargo test --lib                    # Unit tests only
cargo test --test integration       # Integration tests only
cargo test test_download_real_image # Specific test
```

**Run only linting:**
```bash
make lint       # Format check + clippy
```

**Full CI pipeline (format code + lint + test + build):**
```bash
make all
```

All tests use temporary directories (`tempfile` crate) for isolation and automatic cleanup.

### Makefile Targets

```bash
make check     # Run formatting check, linting, and tests
make test      # Run all tests
make lint      # Run clippy and formatting check
make install   # Build release binary and run install script
make clean     # Clean build artifacts
make all       # Format, lint, test, and build release
```

## Dependencies

- `reqwest` - HTTP client with blocking API
- `serde` - Serialization framework
- `chrono` - Date and time handling
- `thiserror` - Error handling

## Uninstall

To remove the installation:

```bash
# Remove binaries
rm ~/.local/bin/natgeo-wallpapers
rm ~/.local/bin/natgeo-set-wallpaper

# Remove systemd timer
systemctl --user stop natgeo-wallpaper.timer
systemctl --user disable natgeo-wallpaper.timer
rm ~/.config/systemd/user/natgeo-wallpaper.service
rm ~/.config/systemd/user/natgeo-wallpaper.timer
systemctl --user daemon-reload

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
- The old JSON API endpoint (`/content/photography/en_US/photo-of-the-day/_jcr_content/.gallery.<date>.json`) is no longer accessible due to CloudFront protection
- This implementation uses HTML scraping as a workaround