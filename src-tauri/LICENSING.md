# Licensing System Documentation

This document describes the comprehensive licensing system implemented for White Space, including secure keychain storage, offline grace periods, and proper UI components.

## Overview

The licensing system provides secure license management with the following features:

- **License Activation**: Activate licenses with license key and instance name
- **License Validation**: Validate licenses every 7 days or on launch
- **Offline Grace Period**: 14-day offline grace period for uninterrupted usage
- **Secure Storage**: License data stored securely in OS keychain
- **Device Management**: Deactivate devices to free up license seats
- **UI Components**: Upgrade dialog and settings integration

## Architecture

### Core Components

1. **LicenseManager**: Handles API communication with license server
2. **LicenseStorage**: Manages secure storage of license data
3. **LicenseChecker**: Validates license status and offline grace periods
4. **Tauri Commands**: Exposes licensing functionality to frontend

### Data Flow

```
Frontend → Tauri Command → LicenseManager → API Server
                ↓
         LicenseStorage → Database (Secure)
                ↓
         LicenseChecker → Validation Logic
```

## API Integration

### License Server Endpoints

All API calls use form parameters (not JSON) and do NOT send secret API keys from the app:

- **POST /v1/activate**: Activate license with key and instance name
- **POST /v1/validate**: Validate license with key and instance ID
- **POST /v1/deactivate**: Deactivate license with key and instance ID

### Request Format

```rust
// Form data sent to API
let mut form_data = HashMap::new();
form_data.insert("license_key".to_string(), license_key.to_string());
form_data.insert("instance_name".to_string(), instance_name.to_string());
```

### Response Types

```rust
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
```

## Secure Storage

### Database Storage

License data is stored securely in the SQLite database using the preferences table:

- **license_key**: The license key (encrypted in production)
- **instance_id**: Unique instance identifier
- **instance_name**: Human-readable instance name
- **last_validated**: Timestamp of last successful validation
- **license_expires_at**: License expiration date
- **license_max_seats**: Maximum number of seats
- **license_used_seats**: Currently used seats

### Storage Methods

```rust
impl LicenseStorage {
    // Store license data
    pub fn store_license_data(&self, license_key: &str, instance_id: &str, instance_name: &str) -> Result<(), String>

    // Retrieve license data
    pub fn get_license_data(&self) -> Result<(Option<String>, Option<String>, Option<String>), String>

    // Clear license data
    pub fn clear_license_data(&self) -> Result<(), String>

    // Store license details
    pub fn store_license_details(&self, expires_at: Option<DateTime<Utc>>, max_seats: Option<u32>, used_seats: Option<u32>) -> Result<(), String>
}
```

## Offline Grace Period

### Logic

The system provides a 14-day offline grace period:

1. **Last Validation**: Track when license was last successfully validated
2. **Grace Period**: 14 days from last validation
3. **Grace Check**: License remains valid during grace period
4. **Expiration**: After grace period, license becomes invalid

### Implementation

```rust
pub fn is_in_offline_grace(&self) -> Result<bool, String> {
    let last_validated = self.storage.get_last_validated()?;

    if let Some(last_validated) = last_validated {
        let grace_period = Duration::days(14);
        let grace_expires = last_validated + grace_period;
        let now = Utc::now();

        if now < grace_expires {
            return Ok(true);
        }
    }

    Ok(false)
}
```

### Validation Schedule

- **On Launch**: Check if validation is needed
- **Every 7 Days**: Automatic validation when online
- **Manual**: User can trigger validation in settings

## Tauri Commands

### Core Commands

#### `ls_activate(license_key: String, instance_name: String) -> Result<ActivateResp, String>`

Activates a license with the provided key and instance name.

**Parameters:**

- `license_key`: The license key from the user
- `instance_name`: Unique name for this device instance

**Returns:**

- `ActivateResp`: Activation result with instance ID and license details

**Security:**

- Validates and sanitizes inputs
- Stores license data securely
- Updates validation timestamp

#### `ls_validate(license_key: String, instance_id: String) -> Result<ValidateResp, String>`

Validates an existing license.

**Parameters:**

- `license_key`: The license key
- `instance_id`: The instance ID from activation

**Returns:**

- `ValidateResp`: Validation result with license status

**Security:**

- Updates last validation timestamp
- Refreshes license details
- Handles validation errors gracefully

#### `ls_deactivate(license_key: String, instance_id: String) -> Result<DeactivateResp, String>`

Deactivates a license, freeing up the seat.

**Parameters:**

- `license_key`: The license key
- `instance_id`: The instance ID to deactivate

**Returns:**

- `DeactivateResp`: Deactivation result

**Security:**

- Clears all local license data
- Frees up license seat on server

### Helper Commands

#### `ls_get_status() -> Result<LicenseStatus, String>`

Gets comprehensive license status including offline grace information.

#### `ls_check_validation_needed() -> Result<bool, String>`

Checks if license validation is needed (every 7 days).

#### `ls_auto_validate() -> Result<ValidateResp, String>`

Automatically validates license using stored credentials.

#### `ls_clear_license() -> Result<(), String>`

Clears all local license data (for troubleshooting).

## License Status

### Status Structure

```rust
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
```

### Status Messages

- **"No license found"**: No license data stored
- **"License valid"**: License is active and valid
- **"License expired"**: License has expired
- **"Offline grace period active"**: In 14-day grace period
- **"License status unknown"**: License data incomplete

## UI Components

### UpgradeDialog

A comprehensive dialog for license management:

**Features:**

- **License Activation**: Enter license key and instance name
- **License Status**: View current license details
- **Device Management**: Deactivate current device
- **Error Handling**: Clear error messages and success feedback
- **Responsive Design**: Works on all screen sizes

**Props:**

```typescript
interface UpgradeDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onActivate: (
    licenseKey: string,
    instanceName: string
  ) => Promise<{ success: boolean; message: string }>;
  onDeactivate: () => Promise<{ success: boolean; message: string }>;
  licenseStatus?: LicenseStatus;
  className?: string;
}
```

### Settings Component

Integrated settings with license management:

**Features:**

- **License Status**: Comprehensive license information
- **Offline Grace**: Clear indication of grace period status
- **Seat Management**: View used vs available seats
- **Refresh**: Manual license status refresh
- **Upgrade Prompt**: Clear upgrade path for unlicensed users

## Usage Examples

### Frontend Integration

```typescript
// Activate license
const handleActivate = async (licenseKey: string, instanceName: string) => {
  try {
    const result = await invoke("ls_activate", {
      licenseKey,
      instanceName,
    });

    if (result.success) {
      console.log("License activated:", result.instance_id);
      // Refresh license status
      await loadLicenseStatus();
    } else {
      console.error("Activation failed:", result.message);
    }

    return result;
  } catch (error) {
    console.error("Activation error:", error);
    return { success: false, message: "Activation failed" };
  }
};

// Check license status
const loadLicenseStatus = async () => {
  try {
    const status = await invoke("ls_get_status");
    setLicenseStatus(status);
  } catch (error) {
    console.error("Failed to load license status:", error);
  }
};

// Validate license
const validateLicense = async () => {
  try {
    const result = await invoke("ls_auto_validate");
    if (result.success && result.valid) {
      console.log("License validated successfully");
    } else {
      console.error("License validation failed:", result.message);
    }
  } catch (error) {
    console.error("Validation error:", error);
  }
};
```

### App Launch Integration

```typescript
// Check license on app launch
useEffect(() => {
  const checkLicense = async () => {
    try {
      // Check if validation is needed
      const needsValidation = await invoke("ls_check_validation_needed");

      if (needsValidation) {
        // Attempt automatic validation
        await validateLicense();
      }

      // Load current status
      await loadLicenseStatus();
    } catch (error) {
      console.error("License check failed:", error);
    }
  };

  checkLicense();
}, []);
```

## Security Considerations

### API Security

- **No Secret Keys**: App never sends secret API keys
- **Form Data**: All requests use form parameters
- **Input Validation**: All inputs are validated and sanitized
- **Error Handling**: Graceful handling of API errors

### Storage Security

- **Encrypted Storage**: License data encrypted in production
- **Local Only**: No sensitive data sent to external services
- **Secure Deletion**: Proper cleanup of license data

### Network Security

- **HTTPS Only**: All API communication over HTTPS
- **Timeout Handling**: Proper timeout and retry logic
- **Offline Support**: Graceful offline operation

## Error Handling

### Common Error Scenarios

1. **Network Errors**: Handle offline scenarios gracefully
2. **Invalid License**: Clear error messages for invalid keys
3. **Expired License**: Proper handling of expired licenses
4. **Server Errors**: Graceful handling of server issues
5. **Storage Errors**: Handle database/storage failures

### Error Recovery

- **Retry Logic**: Automatic retry for transient errors
- **Fallback**: Graceful degradation when services unavailable
- **User Feedback**: Clear error messages and recovery options
- **Logging**: Comprehensive error logging for debugging

## Testing

### Test Coverage

The licensing system includes comprehensive tests:

- **Unit Tests**: Individual component testing
- **Integration Tests**: End-to-end workflow testing
- **Error Tests**: Error handling and edge cases
- **Security Tests**: Input validation and sanitization
- **Storage Tests**: Database operations and data integrity

### Test Scenarios

- **License Activation**: Successful and failed activations
- **License Validation**: Valid and invalid license scenarios
- **Offline Grace**: Grace period calculations and expiration
- **Storage Operations**: Data storage and retrieval
- **Error Handling**: Network and validation errors

Run tests with:

```bash
cargo test licensing
```

## Troubleshooting

### Common Issues

**Activation Fails**

- Check license key format
- Verify internet connection
- Check server status

**Validation Fails**

- Verify license is still valid
- Check instance ID matches
- Ensure server is accessible

**Offline Grace Issues**

- Check last validation timestamp
- Verify grace period calculation
- Ensure proper time synchronization

### Debug Mode

Enable debug logging:

```bash
RUST_LOG=debug cargo run
```

This provides detailed logging of:

- API requests and responses
- License validation attempts
- Storage operations
- Error details

## Future Enhancements

### Planned Features

- **License Transfer**: Move licenses between devices
- **Team Management**: Advanced team license features
- **Usage Analytics**: License usage tracking
- **Auto-renewal**: Automatic license renewal

### Performance Improvements

- **Caching**: License status caching
- **Background Validation**: Non-blocking validation
- **Batch Operations**: Efficient bulk operations
- **Connection Pooling**: Optimized API connections

### Security Enhancements

- **Hardware Binding**: Device-specific license binding
- **Encryption**: Enhanced data encryption
- **Audit Logging**: Comprehensive audit trails
- **Multi-factor**: Additional authentication layers






