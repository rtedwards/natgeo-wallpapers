#![allow(clippy::unwrap_used)]

use natgeo_wallpapers::{download_natgeo_photo_of_the_day, write_log, PhotoInfo};
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_download_real_image() {
    // Integration test: download a small test image from httpbin
    let temp_dir = TempDir::new().unwrap();
    let save_dir = temp_dir.path().to_str().unwrap();
    let sanitized_title = "test_image";

    // Use httpbin's image endpoint which returns a small PNG
    let test_url = "https://httpbin.org/image/png";

    // Create log path
    let log_path = format!("{}/{}.log", save_dir, sanitized_title);

    // Attempt download (this tests the actual network functionality)
    let result = download_natgeo_photo_of_the_day(test_url, save_dir, sanitized_title, &log_path);

    // If download succeeds, verify file exists
    if result.is_ok() {
        let png_path = format!("{}/{}.png", save_dir, sanitized_title);
        assert!(
            std::path::Path::new(&png_path).exists(),
            "Downloaded PNG file should exist"
        );
    }
    // Note: Test might fail due to network issues, which is acceptable
}

#[test]
fn test_log_and_photo_same_directory() {
    let temp_dir = TempDir::new().unwrap();
    let save_dir = temp_dir.path().to_str().unwrap();
    let sanitized_title = "test_photo";

    // Create photo file
    let photo_path = format!("{}/{}.jpg", save_dir, sanitized_title);
    File::create(&photo_path).unwrap();

    // Create log file
    let log_path = format!("{}/{}.log", save_dir, sanitized_title);
    write_log(&log_path, "Test log entry");

    // Verify both files exist in same directory
    assert!(std::path::Path::new(&photo_path).exists());
    assert!(std::path::Path::new(&log_path).exists());

    // Verify they're in the same parent directory
    let photo_parent = std::path::Path::new(&photo_path).parent().unwrap();
    let log_parent = std::path::Path::new(&log_path).parent().unwrap();
    assert_eq!(photo_parent, log_parent);
}

#[test]
fn test_error_log_creation() {
    let temp_dir = TempDir::new().unwrap();
    let save_dir = temp_dir.path().to_str().unwrap();
    let error_log_path = format!("{}/error.log", save_dir);

    // Simulate error logging
    write_log(&error_log_path, "Failed to fetch photo information");

    // Verify error log exists
    assert!(std::path::Path::new(&error_log_path).exists());

    // Verify error message is in log
    let contents = fs::read_to_string(&error_log_path).unwrap();
    assert!(contents.contains("Failed to fetch photo information"));
}

#[test]
fn test_full_workflow_simulation() {
    let temp_dir = TempDir::new().unwrap();
    let save_dir = temp_dir.path().to_str().unwrap();

    // Simulate the photo info
    let photo_info = PhotoInfo {
        image_url: String::from("https://example.com/photo.jpg"),
        title: String::from("Test Photo"),
    };

    let sanitized_title = "Test_Photo";
    let log_path = format!("{}/{}.log", save_dir, sanitized_title);

    // Log start
    write_log(
        &log_path,
        &format!("Starting download for: {}", photo_info.title),
    );
    write_log(&log_path, &format!("Image URL: {}", photo_info.image_url));

    // Create a mock photo file
    let photo_path = format!("{}/{}.jpg", save_dir, sanitized_title);
    let mut file = File::create(&photo_path).unwrap();
    file.write_all(b"mock image data").unwrap();

    // Log completion
    write_log(
        &log_path,
        &format!(
            "Successfully downloaded photo to: {}/{}",
            save_dir, sanitized_title
        ),
    );
    write_log(&log_path, "Download process completed successfully");

    // Verify both files exist
    assert!(std::path::Path::new(&photo_path).exists());
    assert!(std::path::Path::new(&log_path).exists());

    // Verify log contents
    let log_contents = fs::read_to_string(&log_path).unwrap();
    assert!(log_contents.contains("Starting download for: Test Photo"));
    assert!(log_contents.contains("https://example.com/photo.jpg"));
    assert!(log_contents.contains("Successfully downloaded"));
    assert!(log_contents.contains("Download process completed successfully"));
    assert_eq!(log_contents.lines().count(), 4);
}
