use anyhow::{Context, Result};
use serde_json;
use std::fs;
use std::path::Path;

use super::Settings;

const SETTINGS_FILE: &str = "settings.json";

pub fn save_settings(settings: &Settings, storage_dir: &str) -> Result<()> {
    // Create storage directory if it doesn't exist
    fs::create_dir_all(storage_dir)
        .with_context(|| format!("Failed to create storage directory: {}", storage_dir))?;

    let file_path = Path::new(storage_dir).join(SETTINGS_FILE);

    let json_data = serde_json::to_string_pretty(settings)
        .with_context(|| "Failed to serialize settings to JSON")?;

    fs::write(&file_path, json_data)
        .with_context(|| format!("Failed to write settings to file: {:?}", file_path))?;

    tracing::info!("Settings saved to {:?}", file_path);
    Ok(())
}

pub fn load_settings(storage_dir: &str) -> Result<Settings> {
    let file_path = Path::new(storage_dir).join(SETTINGS_FILE);

    if !file_path.exists() {
        return Err(anyhow::anyhow!("Settings file does not exist: {:?}", file_path));
    }

    let json_data = fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read settings file: {:?}", file_path))?;

    let settings: Settings = serde_json::from_str(&json_data)
        .with_context(|| format!("Failed to parse settings JSON from file: {:?}", file_path))?;

    tracing::info!("Settings loaded from {:?}", file_path);
    Ok(settings)
}

pub fn settings_file_exists(storage_dir: &str) -> bool {
    let file_path = Path::new(storage_dir).join(SETTINGS_FILE);
    file_path.exists()
}