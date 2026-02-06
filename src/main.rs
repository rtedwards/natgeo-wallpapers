use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
use natgeo_wallpapers::{
    download_collection, download_natgeo_photo_of_the_day, expand_tilde,
    extract_collection_name_from_url, get_collection_photos, get_current_web_natgeo_gallery,
    sanitize_title, set_wallpapers_with_options, write_log, PhotoError, WallpaperMode,
    PHOTO_SAVE_PATH,
};
use owo_colors::OwoColorize;
use std::fs;
use std::io::{self, Write};
use std::process::Command;

#[derive(Parser)]
#[command(name = "natgeo-wallpapers")]
#[command(about = "National Geographic Photo of the Day downloader and wallpaper setter")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Download today's National Geographic Photo of the Day
    Download,
    /// Set wallpaper(s) from downloaded photos
    Set {
        /// How to distribute wallpapers across monitors/desktops
        #[arg(short, long, value_enum, default_value_t = Mode::Monitors)]
        mode: Mode,

        /// Also set the lock screen wallpaper (KDE Plasma only)
        #[arg(short, long)]
        lock_screen: bool,

        /// Path to a specific photo or directory to use (default: ~/Pictures/NationalGeographic/)
        #[arg(short, long)]
        path: Option<String>,

        /// Select a random photo instead of the newest
        #[arg(short, long)]
        random: bool,
    },
    /// Set up systemd timer, download today's photo, and set wallpaper
    Install {
        /// Time to run daily (HH:MM format, e.g., 02:00) or interval (e.g., 1h, 30m)
        #[arg(short, long)]
        time: Option<String>,

        /// Uninstall the systemd timer
        #[arg(long)]
        uninstall: bool,

        /// Use random photo selection when setting wallpaper
        #[arg(short, long)]
        random: bool,

        /// Path to photos for wallpaper (default: ~/Pictures/NationalGeographic/)
        #[arg(short, long)]
        path: Option<String>,

        /// Also set the lock screen wallpaper (KDE Plasma only)
        #[arg(short, long)]
        lock_screen: bool,
    },
    /// Download photos from a monthly "Best of Photo of the Day" collection
    DownloadCollection {
        /// URL of the collection page
        #[arg(short, long)]
        url: String,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum Mode {
    /// Different wallpaper per physical monitor
    Monitors,
    /// Different wallpaper per virtual desktop (same across monitors)
    VirtualDesktops,
    /// Different wallpaper per monitor x virtual desktop combination
    Both,
}

impl From<Mode> for WallpaperMode {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Monitors => Self::Monitors,
            Mode::VirtualDesktops => Self::VirtualDesktops,
            Mode::Both => Self::Both,
        }
    }
}

fn main() -> Result<(), PhotoError> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Download) => download()?,
        Some(Commands::Set {
            mode,
            lock_screen,
            path,
            random,
        }) => {
            set_wallpapers_with_options(mode.into(), path, random)?;
            if lock_screen {
                set_lock_screen_wallpaper()?;
            }
        }
        Some(Commands::Install {
            time,
            uninstall,
            random,
            path,
            lock_screen,
        }) => {
            if uninstall {
                uninstall_systemd_timer()?;
            } else {
                install_systemd_timer(time, random, path, lock_screen)?;
            }
        }
        Some(Commands::DownloadCollection { url }) => {
            download_collection_cmd(&url)?;
        }
        None => {
            // Default behavior: download (backwards compatibility)
            download()?;
        }
    }

    Ok(())
}

/// Download today's National Geographic Photo of the Day
fn download() -> Result<(), PhotoError> {
    println!("{}", "=== National Geographic Photo Downloader ===".green());
    println!();

    // Get the current date to create a directory for that date
    let today_date = Local::now().format("%d-%m-%Y").to_string();
    let expanded_base_path = expand_tilde(PHOTO_SAVE_PATH);
    let save_dir = format!("{}{}", expanded_base_path, today_date);

    // Create a directory for today's date (if it doesn't exist)
    if let Err(e) = fs::create_dir_all(&save_dir) {
        return Err(PhotoError::File(e));
    }

    // Get the current photo data
    println!("Fetching photo information...");
    let photo_info = match get_current_web_natgeo_gallery() {
        Ok(info) => {
            println!("{} Found: {}", "✓".green(), info.title);
            info
        }
        Err(e) => {
            println!("{} Failed to fetch photo information: {}", "✗".red(), e);
            let log_path = format!("{}/error.log", save_dir);
            let error_msg = format!("Failed to fetch photo information: {}", e);
            write_log(&log_path, &error_msg);
            return Err(e);
        }
    };

    // Sanitize the title to make it a valid filename
    let sanitized_title = sanitize_title(&photo_info.title);
    let log_path = format!("{}/{}.log", save_dir, sanitized_title);

    // Log start of download
    write_log(
        &log_path,
        &format!("Starting download for: {}", photo_info.title),
    );
    write_log(&log_path, &format!("Image URL: {}", photo_info.image_url));

    // Download the photo and save it with the correct extension
    println!("Downloading photo...");
    match download_natgeo_photo_of_the_day(
        &photo_info.image_url,
        &save_dir,
        &sanitized_title,
        &log_path,
    ) {
        Ok(()) => {
            println!(
                "{} Photo saved to: {}/{}",
                "✓".green(),
                save_dir,
                sanitized_title
            );
            let success_msg = format!(
                "Successfully downloaded photo to: {}/{}",
                save_dir, sanitized_title
            );
            write_log(&log_path, &success_msg);
        }
        Err(e) => {
            println!("{} Failed to download photo: {}", "✗".red(), e);
            let error_msg = format!("Failed to download photo: {}", e);
            write_log(&log_path, &error_msg);
            write_log(&log_path, &format!("Error details: {:?}", e));
            return Err(e);
        }
    }

    write_log(&log_path, "Download process completed successfully");

    println!();
    println!("{}", "=== Download Complete ===".green());

    Ok(())
}

/// Download photos from a "Best of Photo of the Day" collection
fn download_collection_cmd(url: &str) -> Result<(), PhotoError> {
    println!(
        "{}",
        "=== National Geographic Collection Downloader ===".green()
    );
    println!();

    // Validate URL contains expected pattern
    if !url.contains("nationalgeographic.com") {
        println!(
            "{} Invalid URL: must be a National Geographic URL",
            "✗".red()
        );
        return Err(PhotoError::InvalidContentType(
            "Invalid URL: must be a National Geographic URL".to_string(),
        ));
    }

    // Fetch the collection
    println!("Fetching collection from: {}", url);
    println!();

    let collection = match get_collection_photos(url) {
        Ok(c) => {
            println!("{} Collection: {}", "✓".green(), c.name);
            println!("{} Found {} photo(s)", "✓".green(), c.photos.len());
            c
        }
        Err(e) => {
            println!("{} Failed to fetch collection: {}", "✗".red(), e);
            return Err(e);
        }
    };

    println!();
    println!("{}", "Photos in collection:".yellow());
    for (i, photo) in collection.photos.iter().enumerate() {
        println!("  {}. {}", i + 1, photo.title);
    }
    println!();

    // Extract collection name from URL for directory
    let collection_name = extract_collection_name_from_url(url);

    // Download the collection
    println!("{}", "Downloading photos...".yellow());
    println!();

    let result = download_collection(&collection, &collection_name)?;

    println!();
    println!("{}", "=== Download Summary ===".green());
    println!("  Downloaded: {}", result.downloaded.to_string().green());
    println!(
        "  Skipped (already exist): {}",
        result.skipped.to_string().yellow()
    );
    if result.failed > 0 {
        println!("  Failed: {}", result.failed.to_string().red());
    }

    let save_path = format!(
        "{}{}",
        expand_tilde(natgeo_wallpapers::COLLECTION_SAVE_PATH),
        collection_name
    );
    println!();
    println!("Photos saved to: {}", save_path.green());

    Ok(())
}

/// Set the lock screen wallpaper (KDE Plasma only)
fn set_lock_screen_wallpaper() -> Result<(), PhotoError> {
    use natgeo_wallpapers::find_all_photos;

    println!();
    println!("{}", "Setting lock screen wallpaper...".yellow());

    // Find the newest photo
    let photos = find_all_photos()?;
    let newest_photo = photos
        .first()
        .ok_or_else(|| PhotoError::Command("No photos found".to_string()))?;

    // Determine which kwriteconfig to use
    let kwriteconfig = if Command::new("which")
        .arg("kwriteconfig6")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        "kwriteconfig6"
    } else if Command::new("which")
        .arg("kwriteconfig5")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        "kwriteconfig5"
    } else {
        println!("{} kwriteconfig not found (KDE Plasma required)", "✗".red());
        return Err(PhotoError::Command("kwriteconfig not found".to_string()));
    };

    let image_url = format!("file://{}", newest_photo.display());

    let output = Command::new(kwriteconfig)
        .args([
            "--file",
            "kscreenlockerrc",
            "--group",
            "Greeter",
            "--group",
            "Wallpaper",
            "--group",
            "org.kde.image",
            "--group",
            "General",
            "--key",
            "Image",
            &image_url,
        ])
        .output()
        .map_err(|e| PhotoError::Command(e.to_string()))?;

    if output.status.success() {
        println!("{} Lock screen wallpaper set", "✓".green());
        println!(
            "  {}",
            "Note: Changes apply on next lock screen activation".yellow()
        );
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!(
            "{} Failed to set lock screen wallpaper: {}",
            "✗".red(),
            stderr
        );
        Err(PhotoError::Command(stderr.to_string()))
    }
}

/// Get the path to the current binary
fn get_binary_path() -> Result<String, PhotoError> {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(PhotoError::File)
}

/// Schedule type for the timer
enum ScheduleType {
    /// Fixed daily time (e.g., "02:00")
    DailyTime(String),
    /// Interval (e.g., "1h", "30m")
    Interval(String),
}

/// Prompt user for time/interval selection
fn prompt_for_schedule() -> Result<ScheduleType, PhotoError> {
    println!("{}", "Setting up systemd timer...".yellow());
    println!();
    println!("When would you like the wallpaper to update?");
    println!("  1) Daily at 02:00 (recommended for daily photo)");
    println!("  2) Every hour (good for random rotation)");
    println!("  3) Every 30 minutes");
    println!("  4) Custom time (HH:MM)");
    println!("  5) Custom interval (e.g., 2h, 15m)");
    println!("  6) Cancel");
    println!();

    loop {
        print!("Enter choice [1-6]: ");
        io::stdout().flush().ok();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(PhotoError::File)?;

        match input.trim() {
            "1" => return Ok(ScheduleType::DailyTime("02:00".to_string())),
            "2" => return Ok(ScheduleType::Interval("1h".to_string())),
            "3" => return Ok(ScheduleType::Interval("30m".to_string())),
            "4" => loop {
                print!("Enter time (HH:MM format, e.g., 22:45): ");
                io::stdout().flush().ok();

                let mut time_input = String::new();
                io::stdin()
                    .read_line(&mut time_input)
                    .map_err(PhotoError::File)?;

                let time = time_input.trim();
                if is_valid_time(time) {
                    return Ok(ScheduleType::DailyTime(time.to_string()));
                }
                println!(
                    "{} Invalid format. Please use HH:MM (00:00-23:59)",
                    "✗".red()
                );
            },
            "5" => loop {
                print!("Enter interval (e.g., 1h, 30m, 2h30m): ");
                io::stdout().flush().ok();

                let mut interval_input = String::new();
                io::stdin()
                    .read_line(&mut interval_input)
                    .map_err(PhotoError::File)?;

                let interval = interval_input.trim();
                if is_valid_interval(interval) {
                    return Ok(ScheduleType::Interval(interval.to_string()));
                }
                println!(
                    "{} Invalid format. Use h for hours, m for minutes (e.g., 1h, 30m, 2h30m)",
                    "✗".red()
                );
            },
            "6" => {
                println!("{} Cancelled", "!".yellow());
                return Err(PhotoError::Command("Cancelled by user".to_string()));
            }
            _ => {
                println!("{} Invalid choice, please enter 1-6", "✗".red());
            }
        }
    }
}

/// Validate time format HH:MM
fn is_valid_time(time: &str) -> bool {
    if time.len() != 5 {
        return false;
    }
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() != 2 {
        return false;
    }
    let hour: Result<u8, _> = parts[0].parse();
    let minute: Result<u8, _> = parts[1].parse();

    matches!((hour, minute), (Ok(h), Ok(m)) if h < 24 && m < 60)
}

/// Validate interval format (e.g., "1h", "30m", "2h30m")
fn is_valid_interval(interval: &str) -> bool {
    if interval.is_empty() {
        return false;
    }
    // Must contain at least one h or m
    if !interval.contains('h') && !interval.contains('m') {
        return false;
    }
    // Check format: optional number+h followed by optional number+m
    let s = interval.to_lowercase();
    let mut has_value = false;

    for c in s.chars() {
        if c.is_ascii_digit() {
            has_value = true;
        } else if c == 'h' || c == 'm' {
            if !has_value {
                return false;
            }
            has_value = false;
        } else {
            return false;
        }
    }
    true
}

/// Parse time or interval from command line argument
fn parse_schedule(time_arg: &str) -> Result<ScheduleType, PhotoError> {
    if is_valid_time(time_arg) {
        Ok(ScheduleType::DailyTime(time_arg.to_string()))
    } else if is_valid_interval(time_arg) {
        Ok(ScheduleType::Interval(time_arg.to_string()))
    } else {
        Err(PhotoError::Command(format!(
            "Invalid time/interval format: {}. Use HH:MM for daily time or intervals like 1h, 30m",
            time_arg
        )))
    }
}

/// Install systemd timer for automatic updates
#[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
fn install_systemd_timer(
    time: Option<String>,
    random: bool,
    path: Option<String>,
    lock_screen: bool,
) -> Result<(), PhotoError> {
    println!("{}", "=== Systemd Timer Setup ===".green());
    println!();

    // Check if systemctl exists
    if Command::new("which")
        .arg("systemctl")
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        println!("{} systemctl not found", "✗".red());
        println!("This feature requires systemd");
        return Err(PhotoError::Command("systemctl not found".to_string()));
    }

    // Get schedule (from argument or prompt)
    let schedule = match time {
        Some(t) => parse_schedule(&t)?,
        None => prompt_for_schedule()?,
    };

    let binary_path = get_binary_path()?;
    let home =
        std::env::var("HOME").map_err(|_| PhotoError::Command("HOME not set".to_string()))?;
    let systemd_dir = format!("{}/.config/systemd/user", home);

    // Create systemd directory
    fs::create_dir_all(&systemd_dir)?;

    // Build the set command with options
    let mut set_args = String::from("set");
    if random {
        set_args.push_str(" --random");
    }
    if let Some(ref p) = path {
        use std::fmt::Write;
        let _ = write!(set_args, " --path '{}'", p);
    }
    if lock_screen {
        set_args.push_str(" --lock-screen");
    }

    // Create service file with the configured options
    let service_content = format!(
        r"[Unit]
Description=Download and set National Geographic Photo of the Day as wallpaper
After=network-online.target network.target
Wants=network-online.target

[Service]
Type=oneshot
ExecStart=/bin/sh -c 'for i in 1 2 3; do {binary} download && {binary} {set_args} && exit 0 || sleep 60; done; exit 1'
",
        binary = binary_path,
        set_args = set_args
    );
    let service_path = format!("{}/natgeo-wallpaper.service", systemd_dir);
    fs::write(&service_path, &service_content)?;
    println!("{} Created {}", "✓".green(), service_path);

    // Create timer file based on schedule type
    let (timer_content, schedule_desc) = match &schedule {
        ScheduleType::DailyTime(time) => {
            let content = format!(
                r"[Unit]
Description=National Geographic Photo of the Day wallpaper update

[Timer]
OnCalendar=*-*-* {}:00
OnBootSec=2min
Persistent=true

[Install]
WantedBy=timers.target
",
                time
            );
            (content, format!("{} daily", time))
        }
        ScheduleType::Interval(interval) => {
            let content = format!(
                r"[Unit]
Description=National Geographic Photo of the Day wallpaper update

[Timer]
OnBootSec=1min
OnUnitActiveSec={}
Persistent=true

[Install]
WantedBy=timers.target
",
                interval
            );
            (content, format!("every {}", interval))
        }
    };

    let timer_path = format!("{}/natgeo-wallpaper.timer", systemd_dir);
    fs::write(&timer_path, timer_content)?;
    println!("{} Created {}", "✓".green(), timer_path);

    // Reload systemd
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output();
    println!("{} Reloaded systemd daemon", "✓".green());

    // Enable timer
    let enable_result = Command::new("systemctl")
        .args(["--user", "enable", "natgeo-wallpaper.timer"])
        .output();

    if enable_result.map(|o| o.status.success()).unwrap_or(false) {
        println!("{} Enabled timer", "✓".green());
    }

    // Start timer
    let start_result = Command::new("systemctl")
        .args(["--user", "start", "natgeo-wallpaper.timer"])
        .output();

    if start_result.map(|o| o.status.success()).unwrap_or(false) {
        println!("{} Started timer", "✓".green());
    }

    println!();
    println!("{}", "=== Timer Setup Complete ===".green());
    println!();
    println!("Schedule: {}", schedule_desc.yellow());
    if random {
        println!("Random selection: {}", "enabled".green());
    }
    if let Some(ref p) = path {
        println!("Photo path: {}", p.green());
    }
    if lock_screen {
        println!("Lock screen: {}", "enabled".green());
    }
    println!();

    // Download and set wallpaper now
    println!(
        "{}",
        "Downloading today's photo and setting wallpaper...".yellow()
    );
    println!();

    download()?;
    println!();
    set_wallpapers_with_options(WallpaperMode::Monitors, path.clone(), random)?;
    if lock_screen {
        set_lock_screen_wallpaper()?;
    }

    println!();
    println!("Useful commands:");
    println!(
        "  {} - Check timer status",
        "systemctl --user status natgeo-wallpaper.timer".green()
    );
    println!(
        "  {} - View logs",
        "journalctl --user -u natgeo-wallpaper.service".green()
    );
    println!(
        "  {} - Uninstall",
        "natgeo-wallpapers install --uninstall".green()
    );

    Ok(())
}

/// Uninstall systemd timer
fn uninstall_systemd_timer() -> Result<(), PhotoError> {
    println!("{}", "=== Uninstalling Systemd Timer ===".green());
    println!();

    let home =
        std::env::var("HOME").map_err(|_| PhotoError::Command("HOME not set".to_string()))?;
    let systemd_dir = format!("{}/.config/systemd/user", home);

    // Stop timer
    let _ = Command::new("systemctl")
        .args(["--user", "stop", "natgeo-wallpaper.timer"])
        .output();
    println!("{} Stopped timer", "✓".green());

    // Disable timer
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "natgeo-wallpaper.timer"])
        .output();
    println!("{} Disabled timer", "✓".green());

    // Remove files
    let service_path = format!("{}/natgeo-wallpaper.service", systemd_dir);
    let timer_path = format!("{}/natgeo-wallpaper.timer", systemd_dir);

    if std::path::Path::new(&service_path).exists() {
        fs::remove_file(&service_path)?;
        println!("{} Removed {}", "✓".green(), service_path);
    }

    if std::path::Path::new(&timer_path).exists() {
        fs::remove_file(&timer_path)?;
        println!("{} Removed {}", "✓".green(), timer_path);
    }

    // Reload systemd
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output();
    println!("{} Reloaded systemd daemon", "✓".green());

    println!();
    println!("{}", "=== Uninstall Complete ===".green());

    Ok(())
}
