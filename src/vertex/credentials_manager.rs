use anyhow::{Result, anyhow};
use serde_json::{Value, Map};
use std::fs;
use std::path::PathBuf;
use rand::seq::SliceRandom;
use glob::glob;

// Rust equivalent of Python vertex/credentials_manager.py

#[derive(Debug, Clone)]
pub struct CredentialManager {
    pub credentials_dir: PathBuf,
}

impl CredentialManager {
    pub fn new(credentials_dir: PathBuf) -> Self {
        Self { credentials_dir }
    }

    /// Parse multiple JSON objects from a string separated by commas.
    /// Format expected: {json_object1},{json_object2},...
    /// Returns a list of parsed JSON objects.
    pub fn parse_multiple_json_credentials(json_str: &str) -> Result<Vec<Value>> {
        if json_str.trim().is_empty() {
            log::debug!("parse_multiple_json_credentials received empty input");
            return Ok(vec![]);
        }

        let mut credentials_list = Vec::new();
        let mut nesting_level = 0;
        let mut current_object_start = None;
        let chars: Vec<char> = json_str.chars().collect();

        for (i, &ch) in chars.iter().enumerate() {
            match ch {
                '{' => {
                    if nesting_level == 0 {
                        current_object_start = Some(i);
                    }
                    nesting_level += 1;
                }
                '}' => {
                    if nesting_level > 0 {
                        nesting_level -= 1;
                        if nesting_level == 0 {
                            if let Some(start) = current_object_start {
                                let json_object_str: String = chars[start..=i].iter().collect();
                                match serde_json::from_str::<Value>(&json_object_str) {
                                    Ok(credentials_info) => {
                                        // Basic validation for service account structure
                                        let required_fields = [
                                            "type", "project_id", "private_key_id", "private_key", "client_email"
                                        ];

                                        if let Value::Object(ref obj) = credentials_info {
                                            let has_all_fields = required_fields.iter().all(|&field| obj.contains_key(field));
                                            if has_all_fields {
                                                credentials_list.push(credentials_info);
                                                log::debug!("Successfully parsed service account credentials");
                                            } else {
                                                log::warn!("Skipping JSON object: missing required service account fields");
                                                log::debug!("Required fields: {:?}", required_fields);
                                                log::debug!("Found fields: {:?}", obj.keys().collect::<Vec<_>>());
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("Failed to parse JSON object: {}", e);
                                        log::debug!("JSON content: {}", json_object_str);
                                    }
                                }
                            }
                            current_object_start = None;
                        }
                    }
                }
                _ => {}
            }
        }

        if !credentials_list.is_empty() {
            log::info!("Successfully parsed {} service account credentials", credentials_list.len());
        } else {
            log::warn!("No valid service account credentials found in the provided JSON string");
        }

        Ok(credentials_list)
    }

    /// Save multiple credentials to separate files
    pub fn save_multiple_credentials_to_files(&self, credentials_list: &[Value]) -> Result<Vec<PathBuf>> {
        if credentials_list.is_empty() {
            log::warn!("No credentials provided to save");
            return Ok(vec![]);
        }

        // Create credentials directory if it doesn't exist
        if !self.credentials_dir.exists() {
            fs::create_dir_all(&self.credentials_dir)?;
            log::info!("Created credentials directory: {:?}", self.credentials_dir);
        }

        let mut saved_files = Vec::new();

        for (index, credentials) in credentials_list.iter().enumerate() {
            let filename = format!("service_account_{}.json", index + 1);
            let file_path = self.credentials_dir.join(&filename);

            let json_str = serde_json::to_string_pretty(credentials)?;
            fs::write(&file_path, json_str)?;

            saved_files.push(file_path.clone());
            log::info!("Saved credentials to: {:?}", file_path);
        }

        log::info!("Successfully saved {} credential files", saved_files.len());
        Ok(saved_files)
    }

    /// Get all credential files from the credentials directory
    pub fn get_all_credential_files(&self) -> Result<Vec<PathBuf>> {
        if !self.credentials_dir.exists() {
            log::warn!("Credentials directory does not exist: {:?}", self.credentials_dir);
            return Ok(vec![]);
        }

        let pattern = self.credentials_dir.join("*.json");
        let pattern_str = pattern.to_str()
            .ok_or_else(|| anyhow!("Invalid path pattern"))?;

        let mut files = Vec::new();
        for entry in glob(pattern_str)? {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        files.push(path);
                    }
                }
                Err(e) => log::error!("Error reading credential file: {}", e),
            }
        }

        files.sort();
        log::debug!("Found {} credential files", files.len());
        Ok(files)
    }

    /// Get a random credential file
    pub fn get_random_credential_file(&self) -> Result<Option<PathBuf>> {
        let files = self.get_all_credential_files()?;

        if files.is_empty() {
            log::warn!("No credential files found in directory: {:?}", self.credentials_dir);
            return Ok(None);
        }

        let mut rng = rand::thread_rng();
        let random_file = files.choose(&mut rng).cloned();

        if let Some(ref file) = random_file {
            log::debug!("Selected random credential file: {:?}", file);
        }

        Ok(random_file)
    }

    /// Load credentials from a specific file
    pub fn load_credentials_from_file(&self, file_path: &PathBuf) -> Result<Value> {
        if !file_path.exists() {
            return Err(anyhow!("Credential file does not exist: {:?}", file_path));
        }

        let content = fs::read_to_string(file_path)?;
        let credentials = serde_json::from_str::<Value>(&content)?;

        log::debug!("Successfully loaded credentials from: {:?}", file_path);
        Ok(credentials)
    }

    /// Validate a credential file
    pub fn validate_credential_file(&self, file_path: &PathBuf) -> Result<bool> {
        match self.load_credentials_from_file(file_path) {
            Ok(credentials) => {
                if let Value::Object(ref obj) = credentials {
                    let required_fields = [
                        "type", "project_id", "private_key_id", "private_key", "client_email"
                    ];

                    let is_valid = required_fields.iter().all(|&field| obj.contains_key(field));

                    if is_valid {
                        log::debug!("Credential file is valid: {:?}", file_path);
                    } else {
                        log::warn!("Credential file is missing required fields: {:?}", file_path);
                    }

                    Ok(is_valid)
                } else {
                    log::warn!("Credential file is not a JSON object: {:?}", file_path);
                    Ok(false)
                }
            }
            Err(e) => {
                log::error!("Failed to validate credential file {:?}: {}", file_path, e);
                Ok(false)
            }
        }
    }

    /// Clean up invalid credential files
    pub fn cleanup_invalid_credentials(&self) -> Result<usize> {
        let files = self.get_all_credential_files()?;
        let mut removed_count = 0;

        for file in files {
            if !self.validate_credential_file(&file)? {
                match fs::remove_file(&file) {
                    Ok(()) => {
                        log::info!("Removed invalid credential file: {:?}", file);
                        removed_count += 1;
                    }
                    Err(e) => {
                        log::error!("Failed to remove invalid credential file {:?}: {}", file, e);
                    }
                }
            }
        }

        if removed_count > 0 {
            log::info!("Cleaned up {} invalid credential files", removed_count);
        } else {
            log::debug!("No invalid credential files found to clean up");
        }

        Ok(removed_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_multiple_json_credentials_empty() {
        let result = CredentialManager::parse_multiple_json_credentials("");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_multiple_json_credentials_valid() {
        let json_str = r#"{"type":"service_account","project_id":"test","private_key_id":"test","private_key":"test","client_email":"test@test.com"}"#;
        let result = CredentialManager::parse_multiple_json_credentials(json_str);
        assert!(result.is_ok());
        let credentials = result.unwrap();
        assert_eq!(credentials.len(), 1);
    }

    #[test]
    fn test_credential_manager_new() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CredentialManager::new(temp_dir.path().to_path_buf());
        assert_eq!(manager.credentials_dir, temp_dir.path());
    }
}