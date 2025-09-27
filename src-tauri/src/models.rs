use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub id: Option<i64>,
    pub path: String,
    pub parent_dir: String,
    pub mime: Option<String>,
    pub size_bytes: i64,
    pub created_at: DateTime<Utc>,
    pub modified_at: Option<DateTime<Utc>>,
    pub accessed_at: Option<DateTime<Utc>>,
    pub last_opened_at: Option<DateTime<Utc>>,
    pub partial_sha1: Option<String>,
    pub sha1: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub is_deleted: bool,
    pub is_staged: bool,
    pub cooloff_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: Option<i64>,
    pub file_id: i64,
    pub action: ActionType,
    pub batch_id: Option<String>,
    pub src_path: Option<String>,
    pub dst_path: Option<String>,
    pub origin: Option<String>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionType {
    #[serde(rename = "archive")]
    Archive,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "restore")]
    Restore,
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionType::Archive => write!(f, "archive"),
            ActionType::Delete => write!(f, "delete"),
            ActionType::Restore => write!(f, "restore"),
        }
    }
}

impl std::str::FromStr for ActionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "archive" => Ok(ActionType::Archive),
            "delete" => Ok(ActionType::Delete),
            "restore" => Ok(ActionType::Restore),
            _ => Err(format!("Invalid action type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preference {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub id: Option<i64>,
    pub metric: String,
    pub value: f64,
    pub context: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWithAction {
    pub file: File,
    pub latest_action: Option<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyTotals {
    pub week_start: DateTime<Utc>,
    pub total_files: i64,
    pub archived_files: i64,
    pub deleted_files: i64,
    pub restored_files: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAgeStats {
    pub age_days: i64,
    pub count: i64,
}

#[derive(Debug, Clone)]
pub struct NewFile {
    pub path: String,
    pub parent_dir: String,
    pub mime: Option<String>,
    pub size_bytes: i64,
    pub created_at: Option<DateTime<Utc>>,
    pub modified_at: Option<DateTime<Utc>>,
    pub accessed_at: Option<DateTime<Utc>>,
    pub partial_sha1: Option<String>,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewAction {
    pub file_id: i64,
    pub action: ActionType,
    pub batch_id: Option<String>,
    pub src_path: Option<String>,
    pub dst_path: Option<String>,
    pub origin: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewMetric {
    pub metric: String,
    pub value: f64,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedRoot {
    pub id: i64,
    pub path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedFileRecord {
    pub id: i64,
    pub file_id: i64,
    pub staged_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub batch_id: Option<String>,
    pub status: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewStagedFile {
    pub file_id: i64,
    pub staged_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub batch_id: Option<String>,
    pub status: String,
    pub note: Option<String>,
}
