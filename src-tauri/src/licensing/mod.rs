use crate::db::Database;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

// License API response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivateResp {
    pub success: bool,
    pub instance_id: Option<String>,
    pub message: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_seats: Option<u32>,
    pub used_seats: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateResp {
    pub success: bool,
    pub valid: bool,
    pub message: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_seats: Option<u32>,
    pub used_seats: Option<u32>,
    pub instance_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeactivateResp {
    pub success: bool,
    pub message: String,
}

// License status for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseStatus {
    pub is_licensed: bool,
    pub license_key: Option<String>,
    pub instance_id: Option<String>,
    pub instance_name: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_seats: Option<u32>,
    pub used_seats: Option<u32>,
    pub last_validated: Option<DateTime<Utc>>,
    pub is_offline_grace: bool,
    pub grace_expires_at: Option<DateTime<Utc>>,
    pub days_remaining: Option<i64>,
    pub status_message: String,
}

// License manager state
pub struct LicenseManager {
    api_base_url: String,
}

impl LicenseManager {
    pub fn new() -> Self {
        Self {
            api_base_url: "https://api.whitespace.app/v1".to_string(),
        }
    }

    // Make API request with form data
    async fn make_api_request<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        form_data: HashMap<String, String>,
    ) -> Result<T, String> {
        let client = reqwest::Client::new();
        let url = format!("{}/{}", self.api_base_url, endpoint);

        let response = client
            .post(&url)
            .form(&form_data)
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        let result: T = response
            .json()
            .await
            .map_err(|e| format!("JSON parse error: {}", e))?;

        Ok(result)
    }

    // Activate license
    pub async fn activate(
        &self,
        license_key: &str,
        instance_name: &str,
    ) -> Result<ActivateResp, String> {
        let mut form_data = HashMap::new();
        form_data.insert("license_key".to_string(), license_key.to_string());
        form_data.insert("instance_name".to_string(), instance_name.to_string());

        self.make_api_request("activate", form_data).await
    }

    // Validate license
    pub async fn validate(
        &self,
        license_key: &str,
        instance_id: &str,
    ) -> Result<ValidateResp, String> {
        let mut form_data = HashMap::new();
        form_data.insert("license_key".to_string(), license_key.to_string());
        form_data.insert("instance_id".to_string(), instance_id.to_string());

        self.make_api_request("validate", form_data).await
    }

    // Deactivate license
    pub async fn deactivate(
        &self,
        license_key: &str,
        instance_id: &str,
    ) -> Result<DeactivateResp, String> {
        let mut form_data = HashMap::new();
        form_data.insert("license_key".to_string(), license_key.to_string());
        form_data.insert("instance_id".to_string(), instance_id.to_string());

        self.make_api_request("deactivate", form_data).await
    }
}

use chrono::Datelike;
use tokio::sync::RwLock;

#[derive(Default, Debug, Clone)]
pub struct LicenseCache {
    pub license_key: Option<String>,
    pub instance_id: Option<String>,
    pub instance_name: Option<String>,
    pub last_validated_at: Option<i64>,
    pub status: Option<String>, // e.g. "valid" | "invalid" | "grace" | "deactivated"
}

// Keychain storage for license data
pub struct LicenseStorage {
    pub cache: RwLock<LicenseCache>,
}

impl LicenseStorage {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(Default::default()),
        }
    }

    // Store license data in cache
    pub async fn store_license_data(
        &self,
        license_key: &str,
        instance_id: &str,
        instance_name: &str,
    ) {
        let mut cache = self.cache.write().await;
        cache.license_key = Some(license_key.to_string());
        cache.instance_id = Some(instance_id.to_string());
        cache.instance_name = Some(instance_name.to_string());
        cache.last_validated_at = Some(now_ts());
        cache.status = Some("valid".to_string());
    }

    // Get license data from cache
    pub async fn get_license_data(&self) -> (Option<String>, Option<String>, Option<String>) {
        let cache = self.cache.read().await;
        (
            cache.license_key.clone(),
            cache.instance_id.clone(),
            cache.instance_name.clone(),
        )
    }

    // Get last validation time
    pub async fn get_last_validated(&self) -> Option<i64> {
        let cache = self.cache.read().await;
        cache.last_validated_at
    }

    // Clear license data
    pub async fn clear_license_data(&self) {
        let mut cache = self.cache.write().await;
        cache.license_key = None;
        cache.instance_id = None;
        cache.instance_name = None;
        cache.last_validated_at = None;
        cache.status = None;
    }

    // Update license status
    pub async fn update_status(&self, status: &str) {
        let mut cache = self.cache.write().await;
        cache.status = Some(status.to_string());
        cache.last_validated_at = Some(now_ts());
    }
}

// Tauri Commands

#[tauri::command]
pub async fn ls_activate(
    license_key: String,
    instance_name: String,
    state: State<'_, LicenseStorage>,
) -> Result<ActivateResp, String> {
    // Validate inputs
    if license_key.trim().is_empty() {
        return Err("License key cannot be empty".to_string());
    }

    if instance_name.trim().is_empty() {
        return Err("Instance name cannot be empty".to_string());
    }

    // Sanitize inputs
    let license_key = license_key.trim().to_string();
    let instance_name = instance_name.trim().to_string();

    // Create license manager and attempt activation
    let manager = LicenseManager::new();
    let response = manager.activate(&license_key, &instance_name).await?;

    if response.success {
        if let Some(instance_id) = &response.instance_id {
            // Store license data in cache
            state
                .store_license_data(&license_key, instance_id, &instance_name)
                .await;
        }
    }

    Ok(response)
}

#[tauri::command]
pub async fn ls_validate(
    license_key: String,
    instance_id: String,
    state: State<'_, LicenseStorage>,
) -> Result<ValidateResp, String> {
    // Validate inputs
    if license_key.trim().is_empty() {
        return Err("License key cannot be empty".to_string());
    }

    if instance_id.trim().is_empty() {
        return Err("Instance ID cannot be empty".to_string());
    }

    // Sanitize inputs
    let license_key = license_key.trim().to_string();
    let instance_id = instance_id.trim().to_string();

    // Create license manager and attempt validation
    let manager = LicenseManager::new();
    let response = manager.validate(&license_key, &instance_id).await?;

    if response.success && response.valid {
        // Update license status in cache
        state.update_status("valid").await;
    }

    Ok(response)
}

#[tauri::command]
pub async fn ls_deactivate(
    license_key: String,
    instance_id: String,
    state: State<'_, LicenseStorage>,
) -> Result<DeactivateResp, String> {
    // Validate inputs
    if license_key.trim().is_empty() {
        return Err("License key cannot be empty".to_string());
    }

    if instance_id.trim().is_empty() {
        return Err("Instance ID cannot be empty".to_string());
    }

    // Sanitize inputs
    let license_key = license_key.trim().to_string();
    let instance_id = instance_id.trim().to_string();

    // Create license manager and attempt deactivation
    let manager = LicenseManager::new();
    let response = manager.deactivate(&license_key, &instance_id).await?;

    if response.success {
        // Clear license data from cache
        state.clear_license_data().await;
    }

    Ok(response)
}

#[tauri::command]
pub async fn ls_get_status(state: State<'_, LicenseStorage>) -> Result<LicenseStatus, String> {
    let cache = state.cache.read().await;

    // Create a basic status response
    let status = LicenseStatus {
        is_licensed: cache.license_key.is_some() && cache.instance_id.is_some(),
        license_key: cache.license_key.clone(),
        instance_id: cache.instance_id.clone(),
        instance_name: cache.instance_name.clone(),
        expires_at: None, // TODO: implement expiration tracking
        max_seats: None,  // TODO: implement seat tracking
        used_seats: None, // TODO: implement seat tracking
        last_validated: cache
            .last_validated_at
            .map(|ts| chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default()),
        is_offline_grace: false, // TODO: implement offline grace logic
        grace_expires_at: None,
        days_remaining: None,
        status_message: cache
            .status
            .clone()
            .unwrap_or_else(|| "No license".to_string()),
    };

    Ok(status)
}

#[tauri::command]
pub async fn ls_check_validation_needed(state: State<'_, LicenseStorage>) -> Result<bool, String> {
    let cache = state.cache.read().await;

    // Check if we have license data
    if cache.license_key.is_none() || cache.instance_id.is_none() {
        return Ok(true); // Need validation if no license data
    }

    // Check if last validation was more than 7 days ago
    if let Some(last_validated) = cache.last_validated_at {
        let now = now_ts();
        let days_since_validation = (now - last_validated) / (24 * 60 * 60);
        Ok(days_since_validation >= 7)
    } else {
        Ok(true) // Never validated, needs validation
    }
}

#[tauri::command]
pub async fn ls_auto_validate(state: State<'_, LicenseStorage>) -> Result<ValidateResp, String> {
    let (license_key, instance_id, _) = state.get_license_data().await;

    if license_key.is_none() || instance_id.is_none() {
        return Err("No license data found".to_string());
    }

    let license_key = license_key.unwrap();
    let instance_id = instance_id.unwrap();

    // Perform validation
    ls_validate(license_key, instance_id, state).await
}

#[tauri::command]
pub async fn ls_clear_license(state: State<'_, LicenseStorage>) -> Result<(), String> {
    state.clear_license_data().await;
    Ok(())
}

// Helper function to create license storage
pub fn create_license_storage() -> LicenseStorage {
    LicenseStorage::new()
}

fn now_ts() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[cfg(test)]
mod tests;
