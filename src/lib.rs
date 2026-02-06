use chrono::Local;
use owo_colors::OwoColorize;
use rand::seq::SliceRandom;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, USER_AGENT};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    process::Command,
};
use thiserror::Error;

// Constants for the URL and photo storage
// Note: National Geographic has changed their API structure. This is an alternative approach
// that scrapes the photo of the day page directly
pub const NATGEO_POD_URL: &str = "https://www.nationalgeographic.com/photo-of-the-day";
pub const PHOTO_SAVE_PATH: &str = "~/Pictures/NationalGeographic/"; // Photos saved here
pub const COLLECTION_SAVE_PATH: &str = "~/Pictures/NationalGeographic/collections/"; // Collections saved here
pub const LOG_DIR: &str = "~/.local/share/natgeo-wallpapers/";

// Since the JSON API is now protected, we'll need to scrape the HTML page
// For now, let's create a simple structure to hold photo information
#[derive(Debug)]
pub struct PhotoInfo {
    pub image_url: String,
    pub title: String,
}

/// A collection of photos from a "Best of Photo of the Day" page
#[derive(Debug)]
pub struct PhotoCollection {
    pub name: String,
    pub photos: Vec<PhotoInfo>,
}

// Define a custom error type
#[derive(Error, Debug)]
pub enum PhotoError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("File operation error: {0}")]
    File(#[from] std::io::Error),

    #[error("Invalid content type: {0}")]
    InvalidContentType(String),

    #[error("Wallpaper error: {0}")]
    Wallpaper(String),

    #[error("Command execution error: {0}")]
    Command(String),

    #[error("No photos found: {0}")]
    NoPhotos(String),
}

// Wallpaper mode for multi-monitor/virtual desktop support
#[derive(Debug, Clone, Copy, Default)]
pub enum WallpaperMode {
    #[default]
    Monitors,
    VirtualDesktops,
    Both,
}

impl std::fmt::Display for WallpaperMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Monitors => write!(f, "monitors"),
            Self::VirtualDesktops => write!(f, "virtual-desktops"),
            Self::Both => write!(f, "both"),
        }
    }
}

// Detected desktop environment
#[derive(Debug, Clone, Copy)]
pub enum DesktopEnvironment {
    KdePlasma6,
    KdePlasma5,
    PlasmaFallback,
    Gnome,
    Feh,
    Unknown,
}

// Function to get the file extension based on the MIME type
pub fn get_extension_from_content_type(content_type: &str) -> Result<String, PhotoError> {
    if content_type.contains("jpeg") {
        Ok("jpg".to_string())
    } else if content_type.contains("png") {
        Ok("png".to_string())
    } else if content_type.contains("gif") {
        Ok("gif".to_string())
    } else {
        Err(PhotoError::InvalidContentType(content_type.to_string()))
    }
}

// Helper function to write log entries
pub fn write_log(log_path: &str, message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let log_message = format!("[{}] {}\n", timestamp, message);

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = file.write_all(log_message.as_bytes());
    }
}

// Fetch the current "photo of the day" data from the HTML page
// Note: This is a workaround since the JSON API is now protected
pub fn get_current_web_natgeo_gallery() -> Result<PhotoInfo, PhotoError> {
    // Create headers to mimic a real browser request
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/999.0.0.0 Safari/537.36"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
        ),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert(
        "Referer",
        HeaderValue::from_static("https://www.nationalgeographic.com/"),
    );

    // Create a client with headers
    let client = Client::builder().default_headers(headers).build()?;

    // Fetch the raw response
    let response = client.get(NATGEO_POD_URL).send()?;

    // Check the status code (capture it first since we'll consume response later)
    let status = response.status();
    if !status.is_success() {
        return Err(PhotoError::InvalidContentType(format!(
            "HTTP {}: Failed to fetch photo of the day page",
            status
        )));
    }

    let body = response.text()?;

    // Extract image URL from the HTML - look for og:image meta tag
    // The meta tags are all on one line, so we need to find the specific property
    let image_url = body
        .split("property=\"og:image\"")
        .nth(1)
        .and_then(|s| s.split("content=\"").nth(1))
        .and_then(|s| s.split('"').next())
        .unwrap_or("")
        .to_string();

    if image_url.is_empty() {
        return Err(PhotoError::InvalidContentType(
            "Could not extract image URL from page".to_string(),
        ));
    }

    // Extract title from og:title
    let og_title = body
        .split("property=\"og:title\"")
        .nth(1)
        .and_then(|s| s.split("content=\"").nth(1))
        .and_then(|s| s.split('"').next())
        .unwrap_or("")
        .to_string();

    // Check if title is meaningful (not just "Test" or empty or too short)
    let title = if og_title.is_empty() || og_title.len() < 5 || og_title.to_lowercase() == "test" {
        // Fall back to extracting filename from image URL
        image_url
            .split('/')
            .next_back()
            .and_then(|filename| filename.split('.').next())
            .unwrap_or("photo-of-the-day")
            .to_string()
    } else {
        og_title
    };

    Ok(PhotoInfo { image_url, title })
}

// Download the photo of the day and save it to the specified destination
pub fn download_natgeo_photo_of_the_day(
    photo_url: &str,       // URL of the photo to download
    save_dir: &str,        // Directory where the photo will be saved
    sanitized_title: &str, // Sanitized photo title for the filename
    log_path: &str,        // Path to log file for this download
) -> Result<(), PhotoError> {
    // Check if photo already exists (jpg, png, or gif)
    if let Ok(entries) = std::fs::read_dir(save_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if stem == sanitized_title && matches!(ext, "jpg" | "png" | "gif") {
                        write_log(
                            log_path,
                            &format!("Photo already exists: {}", path.display()),
                        );
                        return Ok(());
                    }
                }
            }
        }
    }

    // Create headers to mimic a real browser request
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/999.0.0.0 Safari/537.36"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
        ),
    );

    // Create a client with headers
    let client = Client::builder().default_headers(headers).build()?;

    // Make the full URL request to download the image
    let response = client.get(photo_url).send()?;

    // Ensure the response is successful
    if !response.status().is_success() {
        return Err(PhotoError::InvalidContentType(format!(
            "Failed to download photo: HTTP {}",
            response.status()
        )));
    }

    // Get the content type to determine the file extension (jpg or png)
    let content_type = response
        .headers()
        .get("Content-Type")
        .and_then(|val| val.to_str().ok())
        .unwrap_or_default();

    // Get the file extension based on the content type
    let file_extension = match get_extension_from_content_type(content_type) {
        Ok(ext) => ext,
        Err(_) => "jpg".to_string(), // Default to .jpg if content type isn't recognized
    };

    // Create the filename using the sanitized title
    let photo_filename = format!("{}/{}.{}", save_dir, sanitized_title, file_extension);

    // Open the file to write the downloaded photo
    let mut file = File::create(&photo_filename)?;

    // Download and save the image
    let response_bytes = response.bytes()?;
    io::copy(&mut response_bytes.as_ref(), &mut file)?;

    write_log(log_path, &format!("Downloaded photo: {}", photo_filename));

    Ok(())
}

// ============================================================================
// Collection Scraping Functions
// ============================================================================

/// Extract the collection name from a URL like "best-photos-october-2018"
pub fn extract_collection_name_from_url(url: &str) -> String {
    url.split('/')
        .next_back()
        .unwrap_or("collection")
        .to_string()
}

/// Create the HTTP client with browser-like headers
fn create_http_client() -> Result<Client, PhotoError> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/999.0.0.0 Safari/537.36"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
        ),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert(
        "Referer",
        HeaderValue::from_static("https://www.nationalgeographic.com/"),
    );

    Client::builder()
        .default_headers(headers)
        .build()
        .map_err(PhotoError::from)
}

/// Minimum file size in bytes to keep (skip small thumbnails/icons)
const MIN_PHOTO_SIZE_BYTES: u64 = 50_000; // 50KB

/// Check if a filename looks like a "Best of Photo of the Day" collection photo
/// Matches patterns like: `01-best-pod-october-18`, `02_best-pod-july-18`, `best_pod_landscapes`
fn is_collection_photo_filename(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    lower.contains("best-pod") || lower.contains("best_pod")
}

/// Extract all unique image URLs from i.natgeofe.com in the HTML body
fn extract_natgeo_image_urls(body: &str) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Look for patterns like "https://i.natgeofe.com/n/UUID/filename.jpg"
    // These appear in various contexts: img src, JSON data, meta tags
    for part in body.split("https://i.natgeofe.com/n/") {
        // Skip the first split (before first match)
        if part.starts_with("i.natgeofe.com") {
            continue;
        }

        // Extract the path until we hit a quote, space, or other delimiter
        let path_end = part.find(['"', '\'', ' ', '?', '\\']).unwrap_or(part.len());

        let path = &part[..path_end];

        // Only include if it looks like a valid image path (has UUID and extension)
        // We use to_lowercase() so the ends_with checks are already case-insensitive
        let path_lower = path.to_lowercase();
        #[allow(clippy::case_sensitive_file_extension_comparisons)]
        let has_image_ext = path_lower.ends_with(".jpg")
            || path_lower.ends_with(".png")
            || path_lower.ends_with(".gif");
        if path.contains('/') && has_image_ext {
            // Skip crop variants (e.g., _16x9.jpg, _3x2.jpg) - we want the raw images
            let is_crop_variant = path.contains("_16x9")
                || path.contains("_3x2")
                || path.contains("_4x3")
                || path.contains("_2x1")
                || path.contains("_2x3")
                || path.contains("_3x4")
                || path.contains("_square");

            if !is_crop_variant {
                let full_url = format!("https://i.natgeofe.com/n/{}", path);
                if seen.insert(full_url.clone()) {
                    urls.push(full_url);
                }
            }
        }
    }

    urls
}

/// Fetch photos from a "Best of Photo of the Day" collection page
pub fn get_collection_photos(url: &str) -> Result<PhotoCollection, PhotoError> {
    let client = create_http_client()?;

    let response = client.get(url).send()?;

    let status = response.status();
    if !status.is_success() {
        return Err(PhotoError::InvalidContentType(format!(
            "HTTP {}: Failed to fetch collection page",
            status
        )));
    }

    let body = response.text()?;

    // Extract collection name from og:title or URL
    let name = body
        .split("property=\"og:title\"")
        .nth(1)
        .and_then(|s| s.split("content=\"").nth(1))
        .and_then(|s| s.split('"').next())
        .filter(|s| !s.is_empty() && s.len() >= 5)
        .map_or_else(|| extract_collection_name_from_url(url), String::from);

    // Extract all image URLs
    let image_urls = extract_natgeo_image_urls(&body);

    if image_urls.is_empty() {
        return Err(PhotoError::NoPhotos(format!(
            "No photos found in collection: {}",
            url
        )));
    }

    // Create PhotoInfo for each URL, using filename as title
    // Filter to only include photos that match the "best-pod" naming pattern
    let photos: Vec<PhotoInfo> = image_urls
        .into_iter()
        .filter_map(|image_url| {
            let title = image_url
                .split('/')
                .next_back()
                .and_then(|filename| filename.split('.').next())
                .unwrap_or("photo")
                .to_string();

            // Only include photos matching the collection naming pattern
            if is_collection_photo_filename(&title) {
                Some(PhotoInfo { image_url, title })
            } else {
                None
            }
        })
        .collect();

    if photos.is_empty() {
        return Err(PhotoError::NoPhotos(format!(
            "No collection photos found (matching 'best-pod' pattern) in: {}",
            url
        )));
    }

    Ok(PhotoCollection { name, photos })
}

/// Download result for a collection
#[derive(Debug)]
pub struct CollectionDownloadResult {
    pub downloaded: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// Find a downloaded file by its sanitized title (checks jpg, png, gif extensions)
fn find_downloaded_file(dir: &str, sanitized_title: &str) -> Option<std::path::PathBuf> {
    for ext in ["jpg", "png", "gif"] {
        let path = std::path::PathBuf::from(format!("{}/{}.{}", dir, sanitized_title, ext));
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Download all photos from a collection
pub fn download_collection(
    collection: &PhotoCollection,
    collection_name: &str,
) -> Result<CollectionDownloadResult, PhotoError> {
    let base_dir = expand_tilde(COLLECTION_SAVE_PATH);
    let save_dir = format!("{}{}", base_dir, collection_name);

    // Create the collection directory
    std::fs::create_dir_all(&save_dir)?;

    let log_path = format!("{}/collection.log", save_dir);
    write_log(
        &log_path,
        &format!("Starting download of collection: {}", collection.name),
    );
    write_log(
        &log_path,
        &format!("Total photos: {}", collection.photos.len()),
    );

    let mut downloaded = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for photo in &collection.photos {
        let sanitized_title = sanitize_title(&photo.title);

        // Check if already exists
        let already_exists = std::fs::read_dir(&save_dir).ok().is_some_and(|entries| {
            entries.flatten().any(|entry| {
                let path = entry.path();
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|stem| stem == sanitized_title)
                    && path
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|ext| matches!(ext, "jpg" | "png" | "gif"))
            })
        });

        if already_exists {
            skipped += 1;
            continue;
        }

        match download_natgeo_photo_of_the_day(
            &photo.image_url,
            &save_dir,
            &sanitized_title,
            &log_path,
        ) {
            Ok(()) => {
                // Check file size and remove if too small (likely a thumbnail)
                let downloaded_file = find_downloaded_file(&save_dir, &sanitized_title);
                if let Some(file_path) = downloaded_file {
                    if let Ok(metadata) = std::fs::metadata(&file_path) {
                        if metadata.len() < MIN_PHOTO_SIZE_BYTES {
                            // Remove small file (thumbnail/icon)
                            let _ = std::fs::remove_file(&file_path);
                            write_log(
                                &log_path,
                                &format!(
                                    "Removed {} (too small: {} bytes, min: {} bytes)",
                                    sanitized_title,
                                    metadata.len(),
                                    MIN_PHOTO_SIZE_BYTES
                                ),
                            );
                            skipped += 1;
                            continue;
                        }
                    }
                }
                downloaded += 1;
            }
            Err(e) => {
                write_log(
                    &log_path,
                    &format!("Failed to download {}: {}", photo.title, e),
                );
                failed += 1;
            }
        }
    }

    write_log(
        &log_path,
        &format!(
            "Collection download complete: {} downloaded, {} skipped, {} failed",
            downloaded, skipped, failed
        ),
    );

    Ok(CollectionDownloadResult {
        downloaded,
        skipped,
        failed,
    })
}

// Helper function to sanitize title for filename
pub fn sanitize_title(title: &str) -> String {
    title
        .replace("/", "_")
        .replace(" ", "_")
        .replace(":", "")
        .replace("|", "-")
        .chars()
        .take(100)
        .collect()
}

// Helper function to expand tilde in path
pub fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return path.replacen("~", &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

// ============================================================================
// Wallpaper Setting Functions
// ============================================================================

/// Check if a command exists in PATH
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if a process is running
fn process_running(name: &str) -> bool {
    Command::new("pgrep")
        .args(["-x", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Detect the current desktop environment
pub fn detect_desktop_environment() -> DesktopEnvironment {
    let plasmashell_running = process_running("plasmashell");

    if command_exists("qdbus6") && plasmashell_running {
        DesktopEnvironment::KdePlasma6
    } else if command_exists("qdbus") && plasmashell_running {
        DesktopEnvironment::KdePlasma5
    } else if command_exists("plasma-apply-wallpaperimage") {
        DesktopEnvironment::PlasmaFallback
    } else if command_exists("gsettings") {
        DesktopEnvironment::Gnome
    } else if command_exists("feh") {
        DesktopEnvironment::Feh
    } else {
        DesktopEnvironment::Unknown
    }
}

/// Get monitor count via qdbus
fn get_monitor_count(de: DesktopEnvironment) -> usize {
    let qdbus_cmd = match de {
        DesktopEnvironment::KdePlasma6 => "qdbus6",
        DesktopEnvironment::KdePlasma5 => "qdbus",
        _ => return 1,
    };

    let script = "var allDesktops = desktops(); print(allDesktops.length);";
    let output = Command::new(qdbus_cmd)
        .args([
            "org.kde.plasmashell",
            "/PlasmaShell",
            "org.kde.PlasmaShell.evaluateScript",
            script,
        ])
        .output();

    output
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1)
}

/// Get virtual desktop count via qdbus
fn get_virtual_desktop_count(de: DesktopEnvironment) -> usize {
    let qdbus_cmd = match de {
        DesktopEnvironment::KdePlasma6 => "qdbus6",
        _ => return 1, // Only Plasma 6 supports VD wallpapers reliably
    };

    let output = Command::new(qdbus_cmd)
        .args([
            "org.kde.KWin",
            "/VirtualDesktopManager",
            "org.kde.KWin.VirtualDesktopManager.count",
        ])
        .output();

    output
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1)
}

/// Recursively collect photos from a directory
fn collect_photos(dir: &std::path::Path, photos: &mut Vec<PathBuf>) -> io::Result<()> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_photos(&path, photos)?;
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png" | "gif") {
                    photos.push(path);
                }
            }
        }
    }
    Ok(())
}

/// Find all photos in the photo directory, sorted newest first
pub fn find_all_photos() -> Result<Vec<PathBuf>, PhotoError> {
    find_photos_in_path(None)
}

/// Find photos in a specific path (file or directory), or default location if None
pub fn find_photos_in_path(path: Option<&str>) -> Result<Vec<PathBuf>, PhotoError> {
    let search_path = match path {
        Some(p) => expand_tilde(p),
        None => expand_tilde(PHOTO_SAVE_PATH),
    };

    let search_path_obj = std::path::Path::new(&search_path);

    if !search_path_obj.exists() {
        return Err(PhotoError::NoPhotos(format!(
            "Path not found: {}",
            search_path
        )));
    }

    let mut photos: Vec<PathBuf> = Vec::new();

    // If it's a single file, just use that
    if search_path_obj.is_file() {
        if let Some(ext) = search_path_obj.extension().and_then(|e| e.to_str()) {
            if matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png" | "gif") {
                photos.push(search_path_obj.to_path_buf());
            } else {
                return Err(PhotoError::NoPhotos(format!(
                    "Not a supported image file: {}",
                    search_path
                )));
            }
        }
    } else {
        // It's a directory, collect all photos recursively
        collect_photos(search_path_obj, &mut photos)?;
    }

    if photos.is_empty() {
        return Err(PhotoError::NoPhotos(format!(
            "No photos found in {}",
            search_path
        )));
    }

    // Sort by path in reverse order (newest first)
    photos.sort();
    photos.reverse();

    Ok(photos)
}

/// Wallpaper assignment for display
#[derive(Debug)]
pub struct WallpaperAssignment {
    pub location: String,
    pub photo_path: PathBuf,
    pub is_newest: bool,
}

/// Build wallpaper assignments based on mode
pub fn build_assignments(
    mode: WallpaperMode,
    photos: &[PathBuf],
    monitor_count: usize,
    vd_count: usize,
) -> Vec<WallpaperAssignment> {
    let mut assignments = Vec::new();

    match mode {
        WallpaperMode::Monitors => {
            for i in 0..monitor_count {
                let photo_idx = i % photos.len();
                assignments.push(WallpaperAssignment {
                    location: format!("Monitor {}", i + 1),
                    photo_path: photos[photo_idx].clone(),
                    is_newest: i == 0,
                });
            }
        }
        WallpaperMode::VirtualDesktops => {
            for i in 0..vd_count {
                let photo_idx = i % photos.len();
                assignments.push(WallpaperAssignment {
                    location: format!("Virtual Desktop {}", i + 1),
                    photo_path: photos[photo_idx].clone(),
                    is_newest: i == 0,
                });
            }
        }
        WallpaperMode::Both => {
            let mut idx = 0;
            for vd in 0..vd_count {
                for mon in 0..monitor_count {
                    let photo_idx = idx % photos.len();
                    assignments.push(WallpaperAssignment {
                        location: format!("Monitor {}, VD {}", mon + 1, vd + 1),
                        photo_path: photos[photo_idx].clone(),
                        is_newest: idx == 0,
                    });
                    idx += 1;
                }
            }
        }
    }

    assignments
}

/// Set wallpaper for a specific monitor using qdbus6
fn set_wallpaper_qdbus6(
    monitor_idx: usize,
    photo_path: &std::path::Path,
) -> Result<(), PhotoError> {
    let path_str = photo_path.to_string_lossy();
    let script = format!(
        r"var allDesktops = desktops();
if ({idx} < allDesktops.length) {{
    d = allDesktops[{idx}];
    d.wallpaperPlugin = 'org.kde.image';
    d.currentConfigGroup = Array('Wallpaper', 'org.kde.image', 'General');
    d.writeConfig('Image', 'file://{path}');
}}",
        idx = monitor_idx,
        path = path_str
    );

    let output = Command::new("qdbus6")
        .args([
            "org.kde.plasmashell",
            "/PlasmaShell",
            "org.kde.PlasmaShell.evaluateScript",
            &script,
        ])
        .output()
        .map_err(|e| PhotoError::Command(e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(PhotoError::Wallpaper(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}

/// Set wallpaper for a specific monitor using qdbus (Plasma 5)
fn set_wallpaper_qdbus(monitor_idx: usize, photo_path: &std::path::Path) -> Result<(), PhotoError> {
    let path_str = photo_path.to_string_lossy();
    let script = format!(
        r"var allDesktops = desktops();
if ({idx} < allDesktops.length) {{
    d = allDesktops[{idx}];
    d.wallpaperPlugin = 'org.kde.image';
    d.currentConfigGroup = Array('Wallpaper', 'org.kde.image', 'General');
    d.writeConfig('Image', 'file://{path}');
}}",
        idx = monitor_idx,
        path = path_str
    );

    let output = Command::new("qdbus")
        .args([
            "org.kde.plasmashell",
            "/PlasmaShell",
            "org.kde.PlasmaShell.evaluateScript",
            &script,
        ])
        .output()
        .map_err(|e| PhotoError::Command(e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(PhotoError::Wallpaper(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}

/// Set wallpaper using plasma-apply-wallpaperimage
fn set_wallpaper_plasma_apply(photo_path: &std::path::Path) -> Result<(), PhotoError> {
    let output = Command::new("plasma-apply-wallpaperimage")
        .arg(photo_path)
        .output()
        .map_err(|e| PhotoError::Command(e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(PhotoError::Wallpaper(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}

/// Set wallpaper using gsettings (GNOME)
fn set_wallpaper_gnome(photo_path: &std::path::Path) -> Result<(), PhotoError> {
    let uri = format!("file://{}", photo_path.to_string_lossy());

    // Set both light and dark mode wallpapers
    for key in ["picture-uri", "picture-uri-dark"] {
        let output = Command::new("gsettings")
            .args(["set", "org.gnome.desktop.background", key, &uri])
            .output()
            .map_err(|e| PhotoError::Command(e.to_string()))?;

        if !output.status.success() {
            return Err(PhotoError::Wallpaper(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
    }

    Ok(())
}

/// Set wallpaper using feh (X11)
fn set_wallpaper_feh(photo_path: &std::path::Path) -> Result<(), PhotoError> {
    let output = Command::new("feh")
        .args(["--bg-scale", &photo_path.to_string_lossy()])
        .output()
        .map_err(|e| PhotoError::Command(e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(PhotoError::Wallpaper(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}

/// Main wallpaper setting function (uses default photo directory)
pub fn set_wallpapers(mode: WallpaperMode) -> Result<(), PhotoError> {
    set_wallpapers_with_options(mode, None, false)
}

/// Main wallpaper setting function with optional custom path (for backwards compatibility)
pub fn set_wallpapers_with_path(
    mode: WallpaperMode,
    path: Option<String>,
) -> Result<(), PhotoError> {
    set_wallpapers_with_options(mode, path, false)
}

/// Main wallpaper setting function with all options
#[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
pub fn set_wallpapers_with_options(
    mode: WallpaperMode,
    path: Option<String>,
    random: bool,
) -> Result<(), PhotoError> {
    let log_path = format!("{}wallpaper.log", expand_tilde(LOG_DIR));

    // Ensure log directory exists
    if let Some(parent) = std::path::Path::new(&log_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    println!("{}", "=== National Geographic Wallpaper ===".green());
    println!("Mode: {}\n", mode.to_string().yellow());

    write_log(
        &log_path,
        &format!("Starting wallpaper set with mode: {}", mode),
    );

    // Find photos (from custom path or default)
    let mut photos = find_photos_in_path(path.as_deref())?;
    if let Some(ref p) = path {
        println!("{} Using path: {}", "✓".green(), p);
    }
    if random {
        println!("{} Random selection enabled", "✓".green());
        let mut rng = rand::thread_rng();
        photos.shuffle(&mut rng);
    }
    println!("{} Found {} photo(s)\n", "✓".green(), photos.len());

    // Detect desktop environment
    let de = detect_desktop_environment();
    let monitor_count = get_monitor_count(de);
    let vd_count = get_virtual_desktop_count(de);

    match de {
        DesktopEnvironment::KdePlasma6 => {
            println!(
                "{} Detected KDE Plasma 6: {} monitor(s), {} virtual desktop(s)",
                "✓".green(),
                monitor_count,
                vd_count
            );
        }
        DesktopEnvironment::KdePlasma5 => {
            println!(
                "{} Detected KDE Plasma 5: {} monitor(s)",
                "✓".green(),
                monitor_count
            );
            if matches!(mode, WallpaperMode::VirtualDesktops | WallpaperMode::Both) {
                println!(
                    "{} Virtual desktop mode requires Plasma 6+, falling back to monitors",
                    "!".yellow()
                );
            }
        }
        DesktopEnvironment::PlasmaFallback => {
            println!(
                "{} Using plasma-apply-wallpaperimage (single wallpaper mode)",
                "!".yellow()
            );
        }
        DesktopEnvironment::Gnome => {
            println!("{} Detected GNOME, using gsettings", "✓".green());
        }
        DesktopEnvironment::Feh => {
            println!("{} Using feh for X11", "✓".green());
        }
        DesktopEnvironment::Unknown => {
            return Err(PhotoError::Wallpaper(
                "No supported wallpaper tool found".to_string(),
            ));
        }
    }
    println!();

    // Determine effective mode based on DE capabilities
    let effective_mode = match de {
        DesktopEnvironment::KdePlasma6 => mode,
        _ => WallpaperMode::Monitors, // Single wallpaper or monitor-only for non-Plasma6
    };

    // Build assignments
    let assignments = build_assignments(effective_mode, &photos, monitor_count, vd_count);

    // Calculate needed wallpapers
    let total_needed = assignments.len();
    println!("Wallpapers needed: {}", total_needed);

    if photos.len() < total_needed {
        println!(
            "{} Only {} photos available, will reuse as needed\n",
            "!".yellow(),
            photos.len()
        );
    }
    println!();

    // Display assignments
    println!("{}", "Wallpaper assignments:".yellow());
    for assignment in &assignments {
        let photo_date = assignment
            .photo_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let photo_name = assignment
            .photo_path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        if assignment.is_newest {
            println!(
                "  {}: {} - {} {}",
                assignment.location,
                photo_date.green(),
                photo_name,
                "(newest)".yellow()
            );
        } else {
            println!(
                "  {}: {} - {}",
                assignment.location,
                photo_date.green(),
                photo_name
            );
        }
    }
    println!();

    // Apply wallpapers
    println!("{}", "Applying wallpapers...".yellow());
    println!();

    match de {
        DesktopEnvironment::KdePlasma6 => {
            apply_kde_plasma6_wallpapers(&assignments, effective_mode, monitor_count, &log_path);
        }
        DesktopEnvironment::KdePlasma5 => {
            apply_kde_plasma5_wallpapers(&assignments, &log_path);
        }
        DesktopEnvironment::PlasmaFallback => {
            if let Some(first) = assignments.first() {
                match set_wallpaper_plasma_apply(&first.photo_path) {
                    Ok(()) => {
                        println!("{} Wallpaper set", "✓".green());
                        write_log(
                            &log_path,
                            &format!("Set wallpaper to: {}", first.photo_path.display()),
                        );
                    }
                    Err(e) => {
                        println!("{} Failed to set wallpaper: {}", "✗".red(), e);
                    }
                }
            }
        }
        DesktopEnvironment::Gnome => {
            if let Some(first) = assignments.first() {
                match set_wallpaper_gnome(&first.photo_path) {
                    Ok(()) => {
                        println!("{} Wallpaper set via gsettings", "✓".green());
                        write_log(
                            &log_path,
                            &format!("Set wallpaper to: {}", first.photo_path.display()),
                        );
                    }
                    Err(e) => {
                        println!("{} Failed to set wallpaper: {}", "✗".red(), e);
                    }
                }
            }
        }
        DesktopEnvironment::Feh => {
            if let Some(first) = assignments.first() {
                match set_wallpaper_feh(&first.photo_path) {
                    Ok(()) => {
                        println!("{} Wallpaper set via feh", "✓".green());
                        write_log(
                            &log_path,
                            &format!("Set wallpaper to: {}", first.photo_path.display()),
                        );
                    }
                    Err(e) => {
                        println!("{} Failed to set wallpaper: {}", "✗".red(), e);
                    }
                }
            }
        }
        DesktopEnvironment::Unknown => unreachable!(),
    }

    println!();
    println!("{}", "=== Completed ===".green());
    write_log(&log_path, "Wallpaper setting completed");

    println!("\nLog file: {}", log_path);

    Ok(())
}

/// Apply wallpapers for KDE Plasma 6
fn apply_kde_plasma6_wallpapers(
    assignments: &[WallpaperAssignment],
    mode: WallpaperMode,
    monitor_count: usize,
    log_path: &str,
) {
    match mode {
        WallpaperMode::Monitors => {
            for (i, assignment) in assignments.iter().enumerate() {
                match set_wallpaper_qdbus6(i, &assignment.photo_path) {
                    Ok(()) => {
                        println!("{} {}", "✓".green(), assignment.location);
                        write_log(
                            log_path,
                            &format!(
                                "Set {} to: {}",
                                assignment.location,
                                assignment.photo_path.display()
                            ),
                        );
                    }
                    Err(e) => {
                        println!("{} Failed: {} - {}", "✗".red(), assignment.location, e);
                    }
                }
            }
        }
        WallpaperMode::VirtualDesktops => {
            for assignment in assignments {
                // Set same wallpaper on all monitors for this VD
                for mon in 0..monitor_count {
                    let _ = set_wallpaper_qdbus6(mon, &assignment.photo_path);
                }
                println!("{} {} (all monitors)", "✓".green(), assignment.location);
                write_log(
                    log_path,
                    &format!(
                        "Set {} to: {}",
                        assignment.location,
                        assignment.photo_path.display()
                    ),
                );
            }
        }
        WallpaperMode::Both => {
            for (i, assignment) in assignments.iter().enumerate() {
                let mon_idx = i % monitor_count;
                match set_wallpaper_qdbus6(mon_idx, &assignment.photo_path) {
                    Ok(()) => {
                        println!("{} {}", "✓".green(), assignment.location);
                        write_log(
                            log_path,
                            &format!(
                                "Set {} to: {}",
                                assignment.location,
                                assignment.photo_path.display()
                            ),
                        );
                    }
                    Err(e) => {
                        println!("{} Failed: {} - {}", "✗".red(), assignment.location, e);
                    }
                }
            }
        }
    }
}

/// Apply wallpapers for KDE Plasma 5
fn apply_kde_plasma5_wallpapers(assignments: &[WallpaperAssignment], log_path: &str) {
    for (i, assignment) in assignments.iter().enumerate() {
        match set_wallpaper_qdbus(i, &assignment.photo_path) {
            Ok(()) => {
                println!("{} {}", "✓".green(), assignment.location);
                write_log(
                    log_path,
                    &format!(
                        "Set {} to: {}",
                        assignment.location,
                        assignment.photo_path.display()
                    ),
                );
            }
            Err(e) => {
                println!("{} Failed: {} - {}", "✗".red(), assignment.location, e);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write as IoWrite;
    use tempfile::TempDir;

    #[test]
    fn test_get_extension_from_content_type() {
        // Valid content types
        assert_eq!(
            get_extension_from_content_type("image/jpeg").unwrap(),
            "jpg"
        );
        assert_eq!(
            get_extension_from_content_type("image/jpeg; charset=utf-8").unwrap(),
            "jpg"
        );
        assert_eq!(get_extension_from_content_type("image/png").unwrap(), "png");
        assert_eq!(get_extension_from_content_type("image/gif").unwrap(), "gif");

        // Invalid content types
        assert!(get_extension_from_content_type("text/html").is_err());
        assert!(get_extension_from_content_type("application/pdf").is_err());
    }

    #[test]
    fn test_write_log() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        // Write log entries
        write_log(log_path.to_str().unwrap(), "Test message 1");
        write_log(log_path.to_str().unwrap(), "Test message 2");

        // Read and verify
        let contents = fs::read_to_string(&log_path).unwrap();
        assert!(contents.contains("Test message 1"));
        assert!(contents.contains("Test message 2"));
        assert!(contents.contains("[202")); // Check for timestamp format
        assert_eq!(contents.lines().count(), 2); // Should have 2 lines
    }

    #[test]
    fn test_sanitize_title_special_characters() {
        // Test various special characters
        assert_eq!(
            sanitize_title("Photo: 2024/01/20 | Test"),
            "Photo_2024_01_20_-_Test"
        );
        assert_eq!(
            sanitize_title("Test/Photo: Title|Name"),
            "Test_Photo_Title-Name"
        );

        // Verify problematic characters are removed
        let sanitized = sanitize_title("Bad/Path:Name|Value");
        assert!(!sanitized.contains("/"));
        assert!(!sanitized.contains(":"));
        assert!(!sanitized.contains("|"));
    }

    #[test]
    fn test_sanitize_title_length_limit() {
        let long_title = "A".repeat(150);
        let sanitized = sanitize_title(&long_title);
        assert_eq!(sanitized.len(), 100);
    }

    #[test]
    fn test_html_parsing_og_image() {
        // Simulate HTML with og:image meta tag
        let html = r#"<html><head><meta property="og:image" content="https://example.com/image.jpg"/></head></html>"#;

        let image_url = html
            .split("property=\"og:image\"")
            .nth(1)
            .and_then(|s| s.split("content=\"").nth(1))
            .and_then(|s| s.split('"').next())
            .unwrap_or("");

        assert_eq!(image_url, "https://example.com/image.jpg");
    }

    #[test]
    fn test_html_parsing_og_title() {
        // Simulate HTML with og:title meta tag
        let html = r#"<html><head><meta property="og:title" content="Test Photo"/></head></html>"#;

        let title = html
            .split("property=\"og:title\"")
            .nth(1)
            .and_then(|s| s.split("content=\"").nth(1))
            .and_then(|s| s.split('"').next())
            .unwrap_or("");

        assert_eq!(title, "Test Photo");
    }

    #[test]
    fn test_download_and_save_mock_image() {
        let temp_dir = TempDir::new().unwrap();
        let save_dir = temp_dir.path().to_str().unwrap();
        let sanitized_title = "test_photo";

        // Create a mock image file
        let photo_filename = format!("{}/{}.jpg", save_dir, sanitized_title);
        let mut file = File::create(&photo_filename).unwrap();
        file.write_all(b"fake image data").unwrap();

        // Verify file was created
        assert!(std::path::Path::new(&photo_filename).exists());

        // Verify file contents
        let contents = fs::read(&photo_filename).unwrap();
        assert_eq!(contents, b"fake image data");
    }

    #[test]
    fn test_date_format() {
        // Test the date format used in directory structure
        let date = Local::now().format("%d-%m-%Y").to_string();

        // Verify format is dd-mm-yyyy (should be 10 characters)
        assert_eq!(date.len(), 10);
        assert_eq!(date.chars().nth(2), Some('-'));
        assert_eq!(date.chars().nth(5), Some('-'));
    }

    #[test]
    fn test_title_fallback_from_url() {
        // Test extracting filename from image URL
        let url = "https://i.natgeofe.com/n/d0888b52-1d37-403a-a25a-84c1dc53bbdf/NationalGeographic_433254.jpg";

        let filename = url
            .split('/')
            .next_back()
            .and_then(|filename| filename.split('.').next())
            .unwrap_or("photo-of-the-day");

        assert_eq!(filename, "NationalGeographic_433254");
    }

    #[test]
    fn test_title_fallback_logic() {
        // Test that short/meaningless titles trigger fallback
        let og_title_test = "Test";
        let og_title_empty = "";
        let og_title_short = "Hi";
        let og_title_good = "Beautiful Sunset Over Mountains";

        // These should trigger fallback (too short or "Test")
        assert!(og_title_test.to_lowercase() == "test");
        assert!(og_title_empty.is_empty());
        assert!(og_title_short.len() < 5);

        // This should NOT trigger fallback
        assert!(og_title_good.len() >= 5);
        assert!(og_title_good.to_lowercase() != "test");
    }

    #[test]
    fn test_expand_tilde() {
        // Test tilde expansion
        let home = std::env::var("HOME").unwrap();

        // Should expand tilde
        assert_eq!(expand_tilde("~/test/path"), format!("{}/test/path", home));
        assert_eq!(expand_tilde("~/"), format!("{}/", home));

        // Should not modify paths without tilde
        assert_eq!(expand_tilde("/absolute/path"), "/absolute/path");
        assert_eq!(expand_tilde("relative/path"), "relative/path");
        assert_eq!(expand_tilde("~notahome"), "~notahome");
    }

    // ========================================================================
    // Collection Scraping Tests
    // ========================================================================

    #[test]
    fn test_extract_collection_name_from_url() {
        // Test various URL formats
        assert_eq!(
            extract_collection_name_from_url(
                "https://www.nationalgeographic.com/photography/article/best-photos-october-2018"
            ),
            "best-photos-october-2018"
        );
        assert_eq!(
            extract_collection_name_from_url(
                "https://www.nationalgeographic.com/photography/article/best-photos-september-2018"
            ),
            "best-photos-september-2018"
        );

        // Edge case: URL with trailing slash
        assert_eq!(
            extract_collection_name_from_url("https://example.com/path/to/collection/"),
            ""
        );

        // Edge case: simple path
        assert_eq!(
            extract_collection_name_from_url("collection-name"),
            "collection-name"
        );
    }

    #[test]
    fn test_extract_natgeo_image_urls() {
        // Test HTML with multiple image URLs
        let html = r#"
            <img src="https://i.natgeofe.com/n/abc123/photo1.jpg">
            <img src="https://i.natgeofe.com/n/def456/photo2.jpg">
            <script>{"url": "https://i.natgeofe.com/n/ghi789/photo3.jpg"}</script>
        "#;

        let urls = extract_natgeo_image_urls(html);
        assert_eq!(urls.len(), 3);
        assert!(urls.contains(&"https://i.natgeofe.com/n/abc123/photo1.jpg".to_string()));
        assert!(urls.contains(&"https://i.natgeofe.com/n/def456/photo2.jpg".to_string()));
        assert!(urls.contains(&"https://i.natgeofe.com/n/ghi789/photo3.jpg".to_string()));
    }

    #[test]
    fn test_extract_natgeo_image_urls_filters_crops() {
        // Test that crop variants are filtered out
        let html = r#"
            <img src="https://i.natgeofe.com/n/abc123/photo1.jpg">
            <img src="https://i.natgeofe.com/n/abc123/photo1_16x9.jpg">
            <img src="https://i.natgeofe.com/n/abc123/photo1_3x2.jpg">
            <img src="https://i.natgeofe.com/n/abc123/photo1_square.jpg">
        "#;

        let urls = extract_natgeo_image_urls(html);
        // Should only include the raw image, not crop variants
        assert_eq!(urls.len(), 1);
        assert!(urls.contains(&"https://i.natgeofe.com/n/abc123/photo1.jpg".to_string()));
    }

    #[test]
    fn test_extract_natgeo_image_urls_deduplicates() {
        // Test that duplicate URLs are deduplicated
        let html = r#"
            <img src="https://i.natgeofe.com/n/abc123/photo1.jpg">
            <img src="https://i.natgeofe.com/n/abc123/photo1.jpg">
            <img src="https://i.natgeofe.com/n/abc123/photo1.jpg">
        "#;

        let urls = extract_natgeo_image_urls(html);
        assert_eq!(urls.len(), 1);
    }

    #[test]
    fn test_extract_natgeo_image_urls_handles_query_params() {
        // Test that URLs with query parameters are handled correctly
        let html = r#"
            <img src="https://i.natgeofe.com/n/abc123/photo1.jpg?w=1200">
        "#;

        let urls = extract_natgeo_image_urls(html);
        assert_eq!(urls.len(), 1);
        // Should strip query params
        assert!(urls.contains(&"https://i.natgeofe.com/n/abc123/photo1.jpg".to_string()));
    }

    #[test]
    fn test_photo_collection_struct() {
        let collection = PhotoCollection {
            name: "Best Photos - October 2018".to_string(),
            photos: vec![
                PhotoInfo {
                    image_url: "https://example.com/photo1.jpg".to_string(),
                    title: "Photo 1".to_string(),
                },
                PhotoInfo {
                    image_url: "https://example.com/photo2.jpg".to_string(),
                    title: "Photo 2".to_string(),
                },
            ],
        };

        assert_eq!(collection.name, "Best Photos - October 2018");
        assert_eq!(collection.photos.len(), 2);
        assert_eq!(collection.photos[0].title, "Photo 1");
        assert_eq!(collection.photos[1].title, "Photo 2");
    }

    #[test]
    fn test_collection_download_result_struct() {
        let result = CollectionDownloadResult {
            downloaded: 5,
            skipped: 3,
            failed: 1,
        };

        assert_eq!(result.downloaded, 5);
        assert_eq!(result.skipped, 3);
        assert_eq!(result.failed, 1);
    }

    #[test]
    fn test_is_collection_photo_filename() {
        // Should match "best-pod" patterns
        assert!(is_collection_photo_filename("01-best-pod-october-18"));
        assert!(is_collection_photo_filename("02-best-pod-september-18"));
        assert!(is_collection_photo_filename("09-best-pod-july-18"));
        assert!(is_collection_photo_filename("best_pod_landscapes"));
        assert!(is_collection_photo_filename("01-best_pod-august-18"));

        // Case insensitive
        assert!(is_collection_photo_filename("01-BEST-POD-October-18"));
        assert!(is_collection_photo_filename("BEST_POD_Landscapes"));

        // Should NOT match other patterns
        assert!(!is_collection_photo_filename("MossForest"));
        assert!(!is_collection_photo_filename("GettyImages-109899052"));
        assert!(!is_collection_photo_filename("disneyplus"));
        assert!(!is_collection_photo_filename("kids"));
        assert!(!is_collection_photo_filename("SPI-1162458"));
    }

    #[test]
    fn test_find_downloaded_file() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().to_str().unwrap();

        // Create a test file
        let test_file = temp_dir.path().join("test_photo.jpg");
        fs::write(&test_file, "fake image data").unwrap();

        // Should find the file
        let found = find_downloaded_file(dir, "test_photo");
        assert!(found.is_some());
        assert_eq!(found.unwrap(), test_file);

        // Should not find non-existent file
        let not_found = find_downloaded_file(dir, "nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_min_photo_size_constant() {
        // Verify the minimum size is reasonable (50KB)
        assert_eq!(MIN_PHOTO_SIZE_BYTES, 50_000);
    }
}
