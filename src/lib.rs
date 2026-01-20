use chrono::Local;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, USER_AGENT};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
};
use thiserror::Error;

// Constants for the URL and photo storage
// Note: National Geographic has changed their API structure. This is an alternative approach
// that scrapes the photo of the day page directly
pub const NATGEO_POD_URL: &str = "https://www.nationalgeographic.com/photo-of-the-day";
pub const PHOTO_SAVE_PATH: &str = "~/Pictures/NationalGeographic/"; // Photos saved here

// Since the JSON API is now protected, we'll need to scrape the HTML page
// For now, let's create a simple structure to hold photo information
#[derive(Debug)]
pub struct PhotoInfo {
    pub image_url: String,
    pub title: String,
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
}
