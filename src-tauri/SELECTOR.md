# File Selector Implementation

This document describes the intelligent file selector implementation for the White Space application, including scoring algorithms, bucketing logic, and candidate selection.

## Overview

The selector intelligently identifies files for potential cleanup based on multiple factors including size, age, duplicates, and project activity. It uses a sophisticated scoring algorithm to rank candidates and applies daily caps to prevent overwhelming users.

## Architecture

### Core Components

1. **FileScorer** (`selector/scoring.rs`) - Scoring algorithm and normalization
2. **FileSelector** (`selector/mod.rs`) - Bucketing logic and candidate selection
3. **ScoringContext** - Context for scoring decisions

### Key Features

- **Intelligent Scoring**: Multi-factor scoring with normalization
- **Smart Bucketing**: Categorizes files into logical groups
- **Daily Caps**: Prevents overwhelming users with too many suggestions
- **Confidence Scoring**: Indicates reliability of recommendations
- **Preview Hints**: Provides context for each candidate

## Scoring Algorithm

### Formula

```
score = 0.45*norm(size) + 0.25*norm(age_days)
      + 0.20*(dup?1:0) + 0.10*(unopened?1:0)
      - 0.30*(keyword_flag?1:0) - 0.80*(in_git_repo?1:0)
      - 0.70*(recent_sibling_burst?1:0)
```

### Components

**Positive Factors:**

- **Size (45%)**: Larger files get higher scores (log-normalized)
- **Age (25%)**: Older files get higher scores
- **Duplicates (20%)**: Duplicate files get bonus points
- **Unopened (10%)**: Never-opened files get bonus points

**Negative Factors (Penalties):**

- **Keyword Flags (-30%)**: Files in flagged directories (current, project, active, wip, final)
- **Git Repos (-80%)**: Files in Git repositories
- **Recent Burst (-70%)**: Files in directories with recent activity

### Normalization

- **Size**: Log-normalized to handle wide range of file sizes
- **Age**: Linear normalization up to 1 year maximum
- **Score Range**: Clamped to [0, 1] for consistency

## Bucketing System

### Bucket Types

#### Screenshots

- **Criteria**: Name contains "screenshot" OR under `/Screenshots/`
- **Cap**: 5 files per day
- **Rationale**: Screenshots are often temporary and accumulate quickly

#### Big Downloads

- **Criteria**: Under Downloads, size > 100MB, unopened OR age > 30d
- **Cap**: 3 files per day
- **Rationale**: Large downloads consume significant space

#### Old Desktop

- **Criteria**: Under Desktop, age > 14d
- **Cap**: 2 files per day
- **Rationale**: Desktop files are often temporary

#### Duplicates

- **Criteria**: Identical SHA1 hash (skip files > 2GB)
- **Cap**: 2 files per day
- **Rationale**: Duplicates waste space unnecessarily

### Daily Limits

- **Per Bucket**: Individual caps for each bucket type
- **Total Daily**: Maximum 12 candidates per day (mix cap)
- **Configurable**: Limits can be adjusted via `BucketConfig`

## API Reference

### Tauri Commands

#### `daily_candidates(max_total: usize) -> Vec<Candidate>`

Returns top candidates for cleanup based on scoring algorithm.

```typescript
const candidates = await invoke("daily_candidates", { max_total: 10 });
```

**Returns:**

```typescript
interface Candidate {
  file_id: number; // Database file ID
  path: string; // Full file path
  size_bytes: number; // File size in bytes
  reason: string; // Bucket reason (screenshot, big download, etc.)
  score: number; // Calculated score (0-1)
  confidence: number; // Confidence level (0-1)
  preview_hint: string; // Context hints (duplicate, large, old, etc.)
}
```

#### `get_bucket_stats() -> HashMap<String, usize>`

Returns statistics about files in each bucket.

```typescript
const stats = await invoke("get_bucket_stats");
// Returns: { "screenshots": 15, "big_downloads": 8, "old_desktop": 12, "duplicates": 25 }
```

## Confidence Scoring

The confidence score indicates how reliable a recommendation is:

### High Confidence (0.7-1.0)

- Duplicate files
- Large unopened files
- Old files with no activity indicators

### Medium Confidence (0.4-0.7)

- Files with mixed indicators
- Moderate size/age combinations

### Low Confidence (0.0-0.4)

- Files in active projects
- Recently modified files
- Files with keyword flags

## Preview Hints

Preview hints provide quick context about why a file was selected:

- `duplicate` - File has identical SHA1 hash
- `unopened` - File has never been opened
- `large` - File is larger than 100MB
- `old` - File is older than 30 days
- `git-repo` - File is in a Git repository
- `flagged` - File is in a flagged directory
- `recent-activity` - Directory has recent burst activity

## Configuration

### BucketConfig

```rust
struct BucketConfig {
    screenshots_max: usize,    // Default: 5
    big_downloads_max: usize,  // Default: 3
    old_desktop_max: usize,    // Default: 2
    duplicates_max: usize,     // Default: 2
    daily_total_max: usize,    // Default: 12
}
```

### Customization

The selector can be customized by:

- **Adjusting Caps**: Modify bucket limits and daily totals
- **Scoring Weights**: Change the scoring algorithm weights
- **Bucket Criteria**: Modify detection logic for each bucket
- **Confidence Thresholds**: Adjust confidence calculation

## Usage Examples

### Basic Candidate Selection

```rust
use crate::selector::FileSelector;
use crate::db::Database;

let db = Database::open_db("database.db")?;
let selector = FileSelector::new();
let candidates = selector.daily_candidates(10, &db)?;

for candidate in candidates {
    println!("File: {}, Score: {:.2}, Reason: {}",
             candidate.path, candidate.score, candidate.reason);
}
```

### Custom Configuration

```rust
use crate::selector::{FileSelector, BucketConfig};

let mut selector = FileSelector::new();
let config = BucketConfig {
    screenshots_max: 3,
    big_downloads_max: 5,
    old_desktop_max: 1,
    duplicates_max: 4,
    daily_total_max: 8,
};
selector.update_config(config);
```

### Bucket Statistics

```rust
let stats = selector.get_bucket_stats(&db)?;
println!("Screenshots: {}, Big Downloads: {}",
         stats["screenshots"], stats["big_downloads"]);
```

## Testing

### Test Coverage

The test suite covers:

- **Scoring Edge Cases**: Zero values, extreme values, boundary conditions
- **Normalization**: Size and age normalization functions
- **Confidence Calculation**: Various factor combinations
- **Bucket Classification**: Detection logic for each bucket type
- **Duplicate Detection**: SHA1-based duplicate finding
- **Git Repository Detection**: Repository path identification
- **Burst Detection**: Recent activity analysis
- **Candidate Selection**: End-to-end selection process
- **Configuration Limits**: Cap enforcement

### Edge Cases Tested

- **Zero Size/Age**: Proper handling of edge values
- **Very Large Files**: Normalization with extreme sizes
- **Duplicate Groups**: Multiple files with same SHA1
- **Git Repository Paths**: Various repository structures
- **Time Windows**: Burst detection with different time ranges
- **Configuration Limits**: Respecting daily and bucket caps

## Performance Considerations

### Optimization Strategies

- **Efficient Queries**: Database queries optimized for common patterns
- **Caching**: Context information cached between operations
- **Batch Processing**: Multiple files processed together
- **Early Termination**: Stop processing when caps are reached

### Scalability

- **Large Datasets**: Handles thousands of files efficiently
- **Memory Usage**: Minimal memory footprint for large file sets
- **Database Performance**: Optimized queries with proper indexing

## Integration

### Database Integration

The selector integrates with the existing database schema:

- **Files Table**: Source of file metadata
- **Actions Table**: History of file operations
- **Metrics Table**: Performance and usage statistics

### Scanner Integration

Works seamlessly with the file scanner:

- **Fresh Data**: Uses up-to-date file information
- **Context Awareness**: Leverages project detection results
- **Activity Tracking**: Incorporates burst detection data

## Future Enhancements

### Planned Features

- **Machine Learning**: Learn from user decisions to improve scoring
- **User Preferences**: Personalized scoring based on user behavior
- **Smart Scheduling**: Suggest optimal cleanup times
- **Batch Operations**: Support for bulk file operations
- **Advanced Analytics**: Detailed cleanup impact analysis

### Potential Improvements

- **Parallel Processing**: Multi-threaded candidate evaluation
- **Incremental Updates**: Only re-evaluate changed files
- **Smart Caching**: Cache scoring results for better performance
- **Advanced Filtering**: More sophisticated bucket criteria
- **User Feedback**: Learn from user acceptance/rejection patterns






