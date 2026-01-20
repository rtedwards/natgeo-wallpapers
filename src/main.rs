use chrono::Local;
use natgeo_wallpapers::{
    download_natgeo_photo_of_the_day, expand_tilde, get_current_web_natgeo_gallery, sanitize_title,
    write_log, PhotoError, PHOTO_SAVE_PATH,
};
use std::fs;

fn main() -> Result<(), PhotoError> {
    // Get the current date to create a directory for that date
    let today_date = Local::now().format("%d-%m-%Y").to_string();
    let expanded_base_path = expand_tilde(PHOTO_SAVE_PATH);
    let save_dir = format!("{}{}", expanded_base_path, today_date);

    // Create a directory for today's date (if it doesn't exist)
    if let Err(e) = fs::create_dir_all(&save_dir) {
        return Err(PhotoError::File(e));
    }

    // Get the current photo data
    let photo_info = match get_current_web_natgeo_gallery() {
        Ok(info) => info,
        Err(e) => {
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
    match download_natgeo_photo_of_the_day(
        &photo_info.image_url,
        &save_dir,
        &sanitized_title,
        &log_path,
    ) {
        Ok(_) => {
            let success_msg = format!(
                "Successfully downloaded photo to: {}/{}",
                save_dir, sanitized_title
            );
            write_log(&log_path, &success_msg);
        }
        Err(e) => {
            let error_msg = format!("Failed to download photo: {}", e);
            write_log(&log_path, &error_msg);
            write_log(&log_path, &format!("Error details: {:?}", e));
            return Err(e);
        }
    }

    write_log(&log_path, "Download process completed successfully");
    Ok(())
}
