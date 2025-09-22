# Gauge System Implementation

This document describes the comprehensive gauge system for the White Space application, providing real-time tracking of potential, staged, and freed space with configurable reset options.

## Overview

The gauge system provides three key metrics to track file cleanup progress:

- **Potential**: Total size of current daily candidates for cleanup
- **Staged**: Total size of files archived but not deleted within the time window
- **Freed**: Total size of files deleted within the time window

## Architecture

### Core Components

1. **GaugeManager** (`gauge.rs`) - Main gauge computation and management
2. **GaugeState** - Current gauge metrics and window information
3. **GaugeConfig** - Configuration for reset behavior and window settings

### Key Features

- **Real-time Computation**: Automatic recalculation after archive/delete operations
- **Flexible Windows**: Rolling 7-day or tidy day reset options
- **Multiple Actions**: Handles files with multiple actions correctly
- **Edge Case Handling**: Proper window boundary calculations
- **Serialization**: Full JSON serialization support for state and config

## Gauge Metrics

### Potential Today Bytes

Represents the total size of files currently identified as cleanup candidates by the file selector.

```rust
potential_today_bytes: u64
```

**Calculation**: Sum of `size_bytes` from all current daily candidates

### Staged Week Bytes

Represents the total size of files that have been archived but not deleted within the time window.

```rust
staged_week_bytes: u64
```

**Calculation**: Sum of `size_bytes` from files where:

- Latest action is 'archive' within the window
- No later 'delete' action exists

### Freed Week Bytes

Represents the total size of files that have been deleted within the time window.

```rust
freed_week_bytes: u64
```

**Calculation**: Sum of `size_bytes` from files where:

- Latest action is 'delete' within the window

## Window Configuration

### Rolling Window (Default)

Uses a rolling 7-day window that continuously updates based on the current time.

```rust
struct GaugeConfig {
    reset_on_tidy_day: false,
    rolling_window_days: 7,
    // ... other fields
}
```

**Behavior**: Window always covers the last 7 days from the current time.

### Tidy Day Reset

Uses a fixed weekly reset cycle based on a specific day and hour.

```rust
struct GaugeConfig {
    reset_on_tidy_day: true,
    tidy_day: Weekday::Fri,
    tidy_hour: 17,
    // ... other fields
}
```

**Behavior**: Window resets every Friday at 5:00 PM (or configured day/hour).

## Gauge State

### GaugeState Structure

```rust
struct GaugeState {
    pub potential_today_bytes: u64,        // Current cleanup potential
    pub staged_week_bytes: u64,            // Staged files in window
    pub freed_week_bytes: u64,             // Freed files in window
    pub computed_at: DateTime<Utc>,        // When state was computed
    pub window_start: DateTime<Utc>,       // Window start time
    pub window_end: DateTime<Utc>,         // Window end time
}
```

### State Computation

The gauge state is automatically recomputed after:

- Archive operations
- Delete operations
- Undo operations
- Configuration changes

## API Reference

### Tauri Commands

#### `gauge_state() -> GaugeState`

Gets the current gauge state with all metrics.

```typescript
const state = await invoke("gauge_state");
```

**Returns:**

```typescript
interface GaugeState {
  potential_today_bytes: number; // Current cleanup potential
  staged_week_bytes: number; // Staged files in window
  freed_week_bytes: number; // Freed files in window
  computed_at: string; // ISO timestamp
  window_start: string; // ISO timestamp
  window_end: string; // ISO timestamp
}
```

#### `get_gauge_config() -> GaugeConfig`

Gets the current gauge configuration.

```typescript
const config = await invoke("get_gauge_config");
```

**Returns:**

```typescript
interface GaugeConfig {
  reset_on_tidy_day: boolean; // Use tidy day reset
  tidy_day: string; // Day of week (Mon, Tue, etc.)
  tidy_hour: number; // Hour of day (0-23)
  rolling_window_days: number; // Days for rolling window
}
```

#### `update_gauge_config(config: GaugeConfig) -> void`

Updates the gauge configuration.

```typescript
await invoke("update_gauge_config", {
  config: {
    reset_on_tidy_day: true,
    tidy_day: "Fri",
    tidy_hour: 17,
    rolling_window_days: 7,
  },
});
```

#### `get_next_reset_time() -> DateTime<Utc> | null`

Gets the next reset time for tidy day configuration.

```typescript
const nextReset = await invoke("get_next_reset_time");
```

#### `get_window_info() -> (DateTime<Utc>, DateTime<Utc>, string)`

Gets current window bounds and description.

```typescript
const [start, end, description] = await invoke("get_window_info");
```

## Usage Examples

### Basic Gauge State

```rust
use crate::gauge::GaugeManager;
use crate::db::Database;

let db = Database::open_db("database.db")?;
let gauge_manager = GaugeManager::new();
let state = gauge_manager.gauge_state(&db)?;

println!("Potential: {} bytes", state.potential_today_bytes);
println!("Staged: {} bytes", state.staged_week_bytes);
println!("Freed: {} bytes", state.freed_week_bytes);
```

### Configuration Management

```rust
use crate::gauge::{GaugeManager, GaugeConfig};
use chrono::Weekday;

let mut gauge_manager = GaugeManager::new();

// Enable tidy day reset
gauge_manager.set_reset_on_tidy_day(true);
gauge_manager.set_tidy_day(Weekday::Mon);
gauge_manager.set_tidy_hour(9);

// Or use rolling window
gauge_manager.set_reset_on_tidy_day(false);
gauge_manager.set_rolling_window_days(14);
```

### Window Information

```rust
let gauge_manager = GaugeManager::new();
let now = Utc::now();
let (start, end, description) = gauge_manager.get_window_info(now);

println!("Window: {} to {}", start, end);
println!("Description: {}", description);
```

### Next Reset Time

```rust
let gauge_manager = GaugeManager::new();
if let Some(next_reset) = gauge_manager.get_next_reset_time(Utc::now()) {
    println!("Next reset: {}", next_reset);
}
```

## Testing

### Test Coverage

The test suite covers:

- **State Computation**: Gauge state calculation accuracy
- **Window Bounds**: Rolling and tidy day window calculations
- **Edge Cases**: Window boundary conditions
- **Multiple Actions**: Files with multiple actions per file
- **Configuration**: Config updates and validation
- **Serialization**: JSON serialization/deserialization
- **Edge Cases**: Tidy day edge cases and calculations

### Edge Cases Tested

- **Window Edges**: Exact tidy day hour boundaries
- **Multiple Actions**: Files with archive then delete
- **Outside Window**: Actions outside the time window
- **Tidy Day Calculation**: Different weekdays and hours
- **Rolling Window**: Custom day ranges
- **Configuration Updates**: All config field changes

### Integration Tests

- **Database Integration**: Real database operations
- **Action Tracking**: Archive and delete action handling
- **File Management**: File creation and action logging
- **Time Windows**: Various time window scenarios
- **State Persistence**: State serialization and deserialization

## Performance Considerations

### Optimization Strategies

- **Efficient Queries**: Optimized database queries for action tracking
- **Caching**: Gauge state caching between operations
- **Batch Processing**: Multiple file operations processed together
- **Window Calculation**: Efficient time window calculations

### Scalability

- **Large Datasets**: Handles thousands of files efficiently
- **Memory Usage**: Minimal memory footprint for large file sets
- **Database Performance**: Optimized queries with proper indexing
- **Real-time Updates**: Fast state recalculation after operations

## Configuration Options

### Reset Behavior

#### Rolling Window (Default)

- **Window**: Last 7 days from current time
- **Reset**: Continuous rolling window
- **Use Case**: Continuous monitoring

#### Tidy Day Reset

- **Window**: Fixed weekly cycle
- **Reset**: Specific day and hour
- **Use Case**: Weekly cleanup cycles

### Customization

- **Tidy Day**: Any day of the week
- **Tidy Hour**: Any hour (0-23)
- **Rolling Days**: Custom window size
- **Reset Behavior**: Toggle between modes

## Error Handling

### Error Types

- **Database Errors**: Database connection and query failures
- **Configuration Errors**: Invalid configuration values
- **Window Errors**: Invalid time window calculations
- **State Errors**: State computation failures

### Error Recovery

- **Graceful Degradation**: Partial state computation on errors
- **Default Values**: Safe defaults for failed computations
- **Error Logging**: Comprehensive error logging
- **User Feedback**: Clear error messages and suggestions

## Future Enhancements

### Planned Features

- **Historical Tracking**: Long-term gauge history
- **Trend Analysis**: Gauge trend calculations
- **Custom Metrics**: User-defined gauge metrics
- **Notifications**: Gauge threshold notifications
- **Export**: Gauge data export capabilities

### Potential Improvements

- **Real-time Updates**: WebSocket-based real-time updates
- **Advanced Analytics**: Detailed gauge analytics
- **Machine Learning**: Predictive gauge modeling
- **Integration**: Third-party tool integration
- **Visualization**: Advanced gauge visualization

## Integration

### Database Integration

The gauge system integrates with the existing database schema:

- **Files Table**: Source of file metadata and sizes
- **Actions Table**: Action history for staged/freed calculations
- **Metrics Table**: Gauge performance and usage statistics

### Operations Integration

Works seamlessly with the operations system:

- **Archive Operations**: Updates staged metrics
- **Delete Operations**: Updates freed metrics
- **Undo Operations**: Recalculates affected metrics
- **Space Management**: Integrates with space validation

### Selector Integration

Leverages the file selector for potential calculations:

- **Daily Candidates**: Uses current candidate list
- **Bucket Statistics**: Integrates with bucket metrics
- **Scoring**: Incorporates scoring results
- **Configuration**: Shares configuration settings






