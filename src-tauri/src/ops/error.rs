use std::fmt;

#[derive(Debug, Clone)]
pub enum OpsError {
    ArchiveError(String),
    DeleteError(String),
    UndoError(String),
    SpaceError(String),
    PermissionError(String),
    FileNotFound(String),
    InvalidPath(String),
    CrossVolumeError(String),
    BatchError(String),
    DatabaseError(String),
    GaugeError(String),
}

pub type OpsResult<T> = Result<T, OpsError>;

impl fmt::Display for OpsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpsError::ArchiveError(msg) => write!(f, "Archive Error: {}", msg),
            OpsError::DeleteError(msg) => write!(f, "Delete Error: {}", msg),
            OpsError::UndoError(msg) => write!(f, "Undo Error: {}", msg),
            OpsError::SpaceError(msg) => write!(f, "Space Error: {}", msg),
            OpsError::PermissionError(msg) => write!(f, "Permission Error: {}", msg),
            OpsError::FileNotFound(msg) => write!(f, "File Not Found: {}", msg),
            OpsError::InvalidPath(msg) => write!(f, "Invalid Path: {}", msg),
            OpsError::CrossVolumeError(msg) => write!(f, "Cross Volume Error: {}", msg),
            OpsError::BatchError(msg) => write!(f, "Batch Error: {}", msg),
            OpsError::DatabaseError(msg) => write!(f, "Database Error: {}", msg),
            OpsError::GaugeError(msg) => write!(f, "Gauge Error: {}", msg),
        }
    }
}

impl std::error::Error for OpsError {}

impl From<std::io::Error> for OpsError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::PermissionDenied => {
                OpsError::PermissionError(format!("Permission denied: {}", err))
            }
            std::io::ErrorKind::NotFound => {
                OpsError::FileNotFound(format!("File not found: {}", err))
            }
            std::io::ErrorKind::InvalidInput => {
                OpsError::InvalidPath(format!("Invalid path: {}", err))
            }
            _ => OpsError::ArchiveError(format!("IO error: {}", err)),
        }
    }
}

impl From<rusqlite::Error> for OpsError {
    fn from(err: rusqlite::Error) -> Self {
        OpsError::DatabaseError(format!("Database error: {}", err))
    }
}

impl From<Box<dyn std::error::Error>> for OpsError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        OpsError::DatabaseError(format!("Error: {}", err))
    }
}

pub struct ErrorMessage {
    pub title: String,
    pub message: String,
    pub suggestion: Option<String>,
    pub recoverable: bool,
}

impl OpsError {
    pub fn to_user_message(&self) -> ErrorMessage {
        match self {
            OpsError::ArchiveError(msg) => ErrorMessage {
                title: "Archive Failed".to_string(),
                message: format!("Unable to archive files: {}", msg),
                suggestion: Some("Check disk space and permissions, then try again.".to_string()),
                recoverable: true,
            },
            OpsError::DeleteError(msg) => ErrorMessage {
                title: "Delete Failed".to_string(),
                message: format!("Unable to delete files: {}", msg),
                suggestion: Some("Check file permissions and try again.".to_string()),
                recoverable: true,
            },
            OpsError::UndoError(msg) => ErrorMessage {
                title: "Undo Failed".to_string(),
                message: format!("Unable to undo operation: {}", msg),
                suggestion: Some(
                    "Some files may have been moved or deleted outside the application."
                        .to_string(),
                ),
                recoverable: false,
            },
            OpsError::SpaceError(msg) => ErrorMessage {
                title: "Insufficient Space".to_string(),
                message: format!("Not enough disk space: {}", msg),
                suggestion: Some("Free up disk space or choose a different location.".to_string()),
                recoverable: true,
            },
            OpsError::PermissionError(msg) => ErrorMessage {
                title: "Permission Denied".to_string(),
                message: format!("Access denied: {}", msg),
                suggestion: Some("Run as administrator or check file permissions.".to_string()),
                recoverable: true,
            },
            OpsError::FileNotFound(msg) => ErrorMessage {
                title: "File Not Found".to_string(),
                message: format!("File not found: {}", msg),
                suggestion: Some("The file may have been moved or deleted.".to_string()),
                recoverable: false,
            },
            OpsError::InvalidPath(msg) => ErrorMessage {
                title: "Invalid Path".to_string(),
                message: format!("Invalid file path: {}", msg),
                suggestion: Some("Check the file path and try again.".to_string()),
                recoverable: true,
            },
            OpsError::CrossVolumeError(msg) => ErrorMessage {
                title: "Cross Volume Operation".to_string(),
                message: format!("Cannot move across volumes: {}", msg),
                suggestion: Some(
                    "The operation will copy and delete instead of moving.".to_string(),
                ),
                recoverable: true,
            },
            OpsError::BatchError(msg) => ErrorMessage {
                title: "Batch Operation Failed".to_string(),
                message: format!("Batch operation failed: {}", msg),
                suggestion: Some(
                    "Some files in the batch may have failed. Check individual file status."
                        .to_string(),
                ),
                recoverable: true,
            },
            OpsError::DatabaseError(msg) => ErrorMessage {
                title: "Database Error".to_string(),
                message: format!("Database operation failed: {}", msg),
                suggestion: Some("Try restarting the application.".to_string()),
                recoverable: true,
            },
            OpsError::GaugeError(msg) => ErrorMessage {
                title: "Gauge Error".to_string(),
                message: format!("Gauge calculation failed: {}", msg),
                suggestion: Some("Try refreshing the gauge data.".to_string()),
                recoverable: true,
            },
        }
    }

    pub fn is_recoverable(&self) -> bool {
        self.to_user_message().recoverable
    }

    pub fn get_suggestion(&self) -> Option<String> {
        self.to_user_message().suggestion
    }
}

// Convenience functions for common error patterns
pub fn archive_error(msg: &str) -> OpsError {
    OpsError::ArchiveError(msg.to_string())
}

pub fn delete_error(msg: &str) -> OpsError {
    OpsError::DeleteError(msg.to_string())
}

pub fn undo_error(msg: &str) -> OpsError {
    OpsError::UndoError(msg.to_string())
}

pub fn space_error(msg: &str) -> OpsError {
    OpsError::SpaceError(msg.to_string())
}

pub fn permission_error(msg: &str) -> OpsError {
    OpsError::PermissionError(msg.to_string())
}

pub fn file_not_found(msg: &str) -> OpsError {
    OpsError::FileNotFound(msg.to_string())
}

pub fn invalid_path(msg: &str) -> OpsError {
    OpsError::InvalidPath(msg.to_string())
}

pub fn cross_volume_error(msg: &str) -> OpsError {
    OpsError::CrossVolumeError(msg.to_string())
}

pub fn batch_error(msg: &str) -> OpsError {
    OpsError::BatchError(msg.to_string())
}

pub fn database_error(msg: &str) -> OpsError {
    OpsError::DatabaseError(msg.to_string())
}

pub fn gauge_error(msg: &str) -> OpsError {
    OpsError::GaugeError(msg.to_string())
}

// Error context for better debugging
pub struct ErrorContext {
    pub operation: String,
    pub file_path: Option<String>,
    pub batch_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ErrorContext {
    pub fn new(operation: &str) -> Self {
        Self {
            operation: operation.to_string(),
            file_path: None,
            batch_id: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_file_path(mut self, path: &str) -> Self {
        self.file_path = Some(path.to_string());
        self
    }

    pub fn with_batch_id(mut self, batch_id: &str) -> Self {
        self.batch_id = Some(batch_id.to_string());
        self
    }

    pub fn to_string(&self) -> String {
        let mut context = format!("Operation: {}", self.operation);

        if let Some(path) = &self.file_path {
            context.push_str(&format!(", File: {}", path));
        }

        if let Some(batch_id) = &self.batch_id {
            context.push_str(&format!(", Batch: {}", batch_id));
        }

        context.push_str(&format!(
            ", Time: {}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S")
        ));

        context
    }
}

// Error logging utilities
pub fn log_error(error: &OpsError, context: &ErrorContext) {
    let user_message = error.to_user_message();
    let context_str = context.to_string();

    eprintln!("ERROR: {} - {}", user_message.title, user_message.message);
    eprintln!("CONTEXT: {}", context_str);

    if let Some(suggestion) = &user_message.suggestion {
        eprintln!("SUGGESTION: {}", suggestion);
    }

    eprintln!("RECOVERABLE: {}", user_message.recoverable);
}

// Error recovery strategies
pub enum RecoveryStrategy {
    Retry,
    Skip,
    Abort,
    Fallback,
}

pub fn suggest_recovery_strategy(error: &OpsError) -> RecoveryStrategy {
    match error {
        OpsError::SpaceError(_) => RecoveryStrategy::Abort,
        OpsError::PermissionError(_) => RecoveryStrategy::Retry,
        OpsError::FileNotFound(_) => RecoveryStrategy::Skip,
        OpsError::CrossVolumeError(_) => RecoveryStrategy::Fallback,
        OpsError::BatchError(_) => RecoveryStrategy::Skip,
        _ => RecoveryStrategy::Retry,
    }
}
