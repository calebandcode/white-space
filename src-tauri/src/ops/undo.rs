use crate::db::Database;
use crate::models::{Action, ActionType, NewAction};
use crate::ops::error::{OpsError, OpsResult};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct UndoResult {
    pub batch_id: String,
    pub actions_reversed: usize,
    pub files_restored: usize,
    pub duration_ms: u64,
    pub errors: Vec<String>,
    pub rollback_performed: bool,
}

#[derive(Debug, Clone)]
pub struct BatchInfo {
    pub batch_id: String,
    pub action_type: ActionType,
    pub file_count: usize,
    pub created_at: DateTime<Utc>,
    pub actions: Vec<Action>,
}

pub struct UndoManager {
    supported_actions: Vec<ActionType>,
}

impl UndoManager {
    pub fn new() -> Self {
        Self {
            supported_actions: vec![ActionType::Archive, ActionType::Delete],
        }
    }

    pub fn undo_last(&mut self, db: &Database) -> OpsResult<UndoResult> {
        let start_time = std::time::SystemTime::now();

        // Get the most recent batch
        let batch_info = self.get_last_batch(db)?;

        if !self.supported_actions.contains(&batch_info.action_type) {
            return Err(OpsError::UndoError(format!(
                "Cannot undo action type: {:?}",
                batch_info.action_type
            )));
        }

        let mut actions_reversed = 0;
        let mut files_restored = 0;
        let mut errors = Vec::new();
        let mut rollback_performed = false;

        // Attempt to reverse each action in the batch
        for action in &batch_info.actions {
            match self.reverse_action(action, db) {
                Ok(_) => {
                    actions_reversed += 1;
                    files_restored += 1;
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to reverse action {}: {}",
                        action.id.unwrap_or(0),
                        e
                    ));

                    // If any action fails, perform rollback
                    if !rollback_performed {
                        self.rollback_batch(&batch_info, db)?;
                        rollback_performed = true;
                    }
                }
            }
        }

        let duration = start_time
            .elapsed()
            .unwrap_or(std::time::Duration::from_secs(0));
        let duration_ms = duration.as_millis() as u64;

        Ok(UndoResult {
            batch_id: batch_info.batch_id.clone(),
            actions_reversed,
            files_restored,
            duration_ms,
            errors,
            rollback_performed,
        })
    }

    fn get_last_batch(&self, db: &Database) -> OpsResult<BatchInfo> {
        let batch_id = db.get_latest_batch_id()
            .map_err(|e| OpsError::UndoError(format!("Failed to get latest batch: {}", e)))?
            .ok_or_else(|| OpsError::UndoError("No batches found".to_string()))?;
        
        let actions = db.get_actions_by_batch_id(&batch_id)
            .map_err(|e| OpsError::UndoError(format!("Failed to get batch actions: {}", e)))?;
        
        if actions.is_empty() {
            return Err(OpsError::UndoError("Batch has no actions".to_string()));
        }
        
        let action_type = actions[0].action.clone();
        let created_at = actions[0].created_at;
        
        Ok(BatchInfo {
            batch_id,
            action_type,
            file_count: actions.len(),
            created_at,
            actions,
        })
    }

    pub fn undo_batch(&mut self, target_batch_id: &str, db: &Database) -> OpsResult<UndoResult> {
        // Fetch the batch by id and then reuse the same reverse logic as undo_last
        let start_time = std::time::SystemTime::now();

        let batch_info = self.get_batch_by_id(target_batch_id, db)?;

        if !self.supported_actions.contains(&batch_info.action_type) {
            return Err(OpsError::UndoError(format!(
                "Cannot undo action type: {:?}",
                batch_info.action_type
            )));
        }

        let mut actions_reversed = 0;
        let mut files_restored = 0;
        let mut errors = Vec::new();
        let mut rollback_performed = false;

        for action in &batch_info.actions {
            match self.reverse_action(action, db) {
                Ok(_) => {
                    actions_reversed += 1;
                    files_restored += 1;
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to reverse action {}: {}",
                        action.id.unwrap_or(0),
                        e
                    ));

                    if !rollback_performed {
                        self.rollback_batch(&batch_info, db)?;
                        rollback_performed = true;
                    }
                }
            }
        }

        let duration = start_time
            .elapsed()
            .unwrap_or(std::time::Duration::from_secs(0));
        let duration_ms = duration.as_millis() as u64;

        Ok(UndoResult {
            batch_id: batch_info.batch_id.clone(),
            actions_reversed,
            files_restored,
            duration_ms,
            errors,
            rollback_performed,
        })
    }

    fn reverse_action(&self, action: &Action, db: &Database) -> OpsResult<()> {
        match action.action {
            ActionType::Archive => self.restore_from_archive(action),
            ActionType::Delete => self.restore_from_trash(action),
            ActionType::Restore => Err(OpsError::UndoError(
                "Cannot undo restore action".to_string(),
            )),
        }
    }

    fn restore_from_archive(&self, action: &Action) -> OpsResult<()> {
        let src_path = action.dst_path.as_ref().ok_or_else(|| {
            OpsError::UndoError("No destination path for archive action".to_string())
        })?;
        let dst_path = action
            .src_path
            .as_ref()
            .ok_or_else(|| OpsError::UndoError("No source path for archive action".to_string()))?;

        // Check if source still exists (shouldn't for archive)
        if Path::new(dst_path).exists() {
            return Err(OpsError::UndoError(format!(
                "Destination already exists: {}",
                dst_path
            )));
        }

        // Check if archive file exists
        if !Path::new(src_path).exists() {
            return Err(OpsError::UndoError(format!(
                "Archive file not found: {}",
                src_path
            )));
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = Path::new(dst_path).parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    OpsError::UndoError(format!("Failed to create parent directory: {}", e))
                })?;
            }
        }

        // Move file back to original location
        fs::rename(src_path, dst_path)
            .map_err(|e| OpsError::UndoError(format!("Failed to restore from archive: {}", e)))?;

        Ok(())
    }

    fn restore_from_trash(&self, action: &Action) -> OpsResult<()> {
        let src_path = action.dst_path.as_ref().ok_or_else(|| {
            OpsError::UndoError("No destination path for delete action".to_string())
        })?;
        let dst_path = action
            .src_path
            .as_ref()
            .ok_or_else(|| OpsError::UndoError("No source path for delete action".to_string()))?;

        // Check if destination already exists
        if Path::new(dst_path).exists() {
            return Err(OpsError::UndoError(format!(
                "Destination already exists: {}",
                dst_path
            )));
        }

        // Check if trash file exists
        if !Path::new(src_path).exists() {
            return Err(OpsError::UndoError(format!(
                "Trash file not found: {}",
                src_path
            )));
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = Path::new(dst_path).parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    OpsError::UndoError(format!("Failed to create parent directory: {}", e))
                })?;
            }
        }

        // Move file back to original location
        fs::rename(src_path, dst_path)
            .map_err(|e| OpsError::UndoError(format!("Failed to restore from trash: {}", e)))?;

        Ok(())
    }

    fn rollback_batch(&self, batch_info: &BatchInfo, db: &Database) -> OpsResult<()> {
        // Rollback all successfully moved files in this batch
        for action in &batch_info.actions {
            // Only rollback if the action was successful (file was moved)
            if self.was_action_successful(action) {
                if let Err(e) = self.reverse_action(action, db) {
                    eprintln!(
                        "Failed to rollback action {}: {}",
                        action.id.unwrap_or(0),
                        e
                    );
                }
            }
        }

        Ok(())
    }

    fn was_action_successful(&self, action: &Action) -> bool {
        // Check if the destination file exists (indicating successful move)
        if let Some(dst_path) = &action.dst_path {
            Path::new(dst_path).exists()
        } else {
            false
        }
    }

    pub fn log_restore_action(&self, action: &Action, db: &Database) -> OpsResult<()> {
        let restore_action = NewAction {
            file_id: action.file_id,
            action: ActionType::Restore,
            batch_id: action.batch_id.clone(),
            src_path: action.dst_path.clone(),
            dst_path: action.src_path.clone(),
        };

        db.insert_action(&restore_action)
            .map_err(|e| OpsError::UndoError(format!("Failed to log restore action: {}", e)))?;

        Ok(())
    }

    pub fn get_undoable_batches(&self, db: &Database) -> OpsResult<Vec<BatchInfo>> {
        let batch_ids = db.get_undoable_batches()
            .map_err(|e| OpsError::UndoError(format!("Failed to get undoable batches: {}", e)))?;
        
        let mut batches = Vec::new();
        for batch_id in batch_ids {
            match self.get_batch_by_id(&batch_id, db) {
                Ok(batch_info) => batches.push(batch_info),
                Err(e) => {
                    // Log error but continue with other batches
                    eprintln!("Failed to get batch {}: {}", batch_id, e);
                }
            }
        }
        
        Ok(batches)
    }

    pub fn can_undo_batch(&self, batch_id: &str, db: &Database) -> OpsResult<bool> {
        // Check if all files in the batch can be restored
        let batch_info = self.get_batch_by_id(batch_id, db)?;

        for action in &batch_info.actions {
            if !self.can_restore_action(action) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub fn get_batch_by_id(&self, batch_id: &str, db: &Database) -> OpsResult<BatchInfo> {
        let actions = db.get_actions_by_batch_id(batch_id)
            .map_err(|e| OpsError::UndoError(format!("Failed to get batch actions: {}", e)))?;
        
        if actions.is_empty() {
            return Err(OpsError::UndoError(format!("Batch {} not found", batch_id)));
        }
        
        let action_type = actions[0].action.clone();
        let created_at = actions[0].created_at;
        
        Ok(BatchInfo {
            batch_id: batch_id.to_string(),
            action_type,
            file_count: actions.len(),
            created_at,
            actions,
        })
    }

    fn can_restore_action(&self, action: &Action) -> bool {
        match action.action {
            ActionType::Archive => {
                // Can restore if archive file exists and destination doesn't
                if let (Some(src_path), Some(dst_path)) = (&action.dst_path, &action.src_path) {
                    Path::new(src_path).exists() && !Path::new(dst_path).exists()
                } else {
                    false
                }
            }
            ActionType::Delete => {
                // Can restore if trash file exists and destination doesn't
                if let (Some(src_path), Some(dst_path)) = (&action.dst_path, &action.src_path) {
                    Path::new(src_path).exists() && !Path::new(dst_path).exists()
                } else {
                    false
                }
            }
            ActionType::Restore => false, // Cannot undo restore actions
        }
    }

    pub fn get_restore_preview(&self, batch_id: &str, db: &Database) -> OpsResult<Vec<String>> {
        let batch_info = self.get_batch_by_id(batch_id, db)?;
        let mut preview = Vec::new();

        for action in &batch_info.actions {
            if let (Some(src_path), Some(dst_path)) = (&action.src_path, &action.dst_path) {
                match action.action {
                    ActionType::Archive => {
                        preview.push(format!("Restore {} from archive", dst_path));
                    }
                    ActionType::Delete => {
                        preview.push(format!("Restore {} from trash", dst_path));
                    }
                    ActionType::Restore => {
                        preview.push(format!("Cannot undo restore of {}", dst_path));
                    }
                }
            }
        }

        Ok(preview)
    }
}

impl Default for UndoManager {
    fn default() -> Self {
        Self::new()
    }
}
