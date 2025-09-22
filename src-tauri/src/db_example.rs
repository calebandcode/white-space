// Example usage of the database functions
// This file demonstrates how to use the database API

use crate::db::Database;
use crate::models::*;
use std::path::Path;

pub fn example_usage() -> Result<(), Box<dyn std::error::Error>> {
    // Open database
    let db = Database::open_db("example.db")?;
    
    // Run migrations
    db.run_migrations()?;
    
    // Insert a file
    let new_file = NewFile {
        path: "/home/user/document.pdf".to_string(),
        parent_dir: "/home/user".to_string(),
        mime: Some("application/pdf".to_string()),
        size_bytes: 1024,
        sha1: Some("abc123def456".to_string()),
    };
    let file_id = db.insert_file(new_file)?;
    println!("Inserted file with ID: {}", file_id);
    
    // Insert an action
    let new_action = NewAction {
        file_id,
        action: ActionType::Archive,
        batch_id: Some("batch_001".to_string()),
        src_path: Some("/home/user/document.pdf".to_string()),
        dst_path: Some("/archive/document.pdf".to_string()),
    };
    let action_id = db.insert_action(new_action)?;
    println!("Inserted action with ID: {}", action_id);
    
    // Insert a metric
    let new_metric = NewMetric {
        metric: "files_processed".to_string(),
        value: 1.0,
        context: Some("daily_scan".to_string()),
    };
    let metric_id = db.insert_metric(new_metric)?;
    println!("Inserted metric with ID: {}", metric_id);
    
    // Query files by age (older than 30 days)
    let old_files = db.by_age(30, None)?;
    println!("Found {} old files", old_files.len());
    
    // Query files by directory
    let user_files = db.by_dir("/home/user")?;
    println!("Found {} files in /home/user", user_files.len());
    
    // Get latest action for a file
    if let Some(action) = db.latest_action(file_id)? {
        println!("Latest action: {:?}", action);
    }
    
    // Get weekly totals
    let weekly_stats = db.weekly_totals(4)?; // Last 4 weeks
    for stats in weekly_stats {
        println!("Week {}: {} files processed", stats.week_start, stats.total_files);
    }
    
    // Set a preference
    db.set_preference("last_scan", "2024-01-01T00:00:00Z")?;
    
    // Get a preference
    if let Some(value) = db.get_preference("last_scan")? {
        println!("Last scan: {}", value);
    }
    
    Ok(())
}






