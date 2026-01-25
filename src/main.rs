use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
use natgeo_wallpapers::{
    download_natgeo_photo_of_the_day, expand_tilde, get_current_web_natgeo_gallery, sanitize_title,
    set_wallpapers, write_log, PhotoError, WallpaperMode, PHOTO_SAVE_PATH,
};
use owo_colors::OwoColorize;
use std::fs;
use std::io::{self, Write};
use std::process::Command;

const SERVICE_TEMPLATE: &str = include_str!("../assets/service.template");
const TIMER_TEMPLATE: &str = include_str!("../assets/timer.template");

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
    },
    /// Set up systemd timer, download today's photo, and set wallpaper
    Install {
        /// Time to run daily (HH:MM format, e.g., 02:00)
        #[arg(short, long)]
        time: Option<String>,

        /// Uninstall the systemd timer
        #[arg(long)]
        uninstall: bool,
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
        Some(Commands::Set { mode, lock_screen }) => {
            set_wallpapers(mode.into())?;
            if lock_screen {
                set_lock_screen_wallpaper()?;
            }
        }
        Some(Commands::Install { time, uninstall }) => {
            if uninstall {
                uninstall_systemd_timer()?;
            } else {
                install_systemd_timer(time)?;
            }
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

/// Prompt user for time selection
fn prompt_for_time() -> Result<String, PhotoError> {
    println!("{}", "Setting up systemd timer...".yellow());
    println!();
    println!("When would you like the wallpaper to update?");
    println!("  1) Daily at 02:00 (recommended)");
    println!("  2) Daily at 06:00");
    println!("  3) Daily at 08:00");
    println!("  4) Custom time");
    println!("  5) Cancel");
    println!();

    loop {
        print!("Enter choice [1-5]: ");
        io::stdout().flush().ok();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(PhotoError::File)?;

        match input.trim() {
            "1" => return Ok("02:00".to_string()),
            "2" => return Ok("06:00".to_string()),
            "3" => return Ok("08:00".to_string()),
            "4" => loop {
                print!("Enter time (HH:MM format, e.g., 22:45): ");
                io::stdout().flush().ok();

                let mut time_input = String::new();
                io::stdin()
                    .read_line(&mut time_input)
                    .map_err(PhotoError::File)?;

                let time = time_input.trim();
                if is_valid_time(time) {
                    return Ok(time.to_string());
                }
                println!(
                    "{} Invalid format. Please use HH:MM (00:00-23:59)",
                    "✗".red()
                );
            },
            "5" => {
                println!("{} Cancelled", "!".yellow());
                return Err(PhotoError::Command("Cancelled by user".to_string()));
            }
            _ => {
                println!("{} Invalid choice, please enter 1-5", "✗".red());
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

/// Install systemd timer for automatic updates
fn install_systemd_timer(time: Option<String>) -> Result<(), PhotoError> {
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

    // Get time (from argument or prompt)
    let schedule_time = match time {
        Some(t) => {
            if !is_valid_time(&t) {
                println!("{} Invalid time format: {}", "✗".red(), t);
                return Err(PhotoError::Command(format!("Invalid time format: {}", t)));
            }
            t
        }
        None => prompt_for_time()?,
    };

    let binary_path = get_binary_path()?;
    let home =
        std::env::var("HOME").map_err(|_| PhotoError::Command("HOME not set".to_string()))?;
    let systemd_dir = format!("{}/.config/systemd/user", home);

    // Create systemd directory
    fs::create_dir_all(&systemd_dir)?;

    // Create service file
    let service_content = SERVICE_TEMPLATE.replace("{{BINARY}}", &binary_path);
    let service_path = format!("{}/natgeo-wallpaper.service", systemd_dir);
    fs::write(&service_path, service_content)?;
    println!("{} Created {}", "✓".green(), service_path);

    // Create timer file
    let timer_content = TIMER_TEMPLATE.replace("{{TIME}}", &schedule_time);
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
    println!("Timer scheduled: {} daily", schedule_time.yellow());
    println!("Also runs 2 minutes after boot");
    println!();

    // Download and set wallpaper now
    println!(
        "{}",
        "Downloading today's photo and setting wallpaper...".yellow()
    );
    println!();

    download()?;
    println!();
    set_wallpapers(WallpaperMode::Monitors)?;

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
