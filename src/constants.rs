//! Shared constants used across runtime, tools, and services.

/// Default fallback soul when no configured soul is available.
pub const DEFAULT_SOUL_FALLBACK: &str = "You are BoBe, a helpful AI assistant.";

/// Milliseconds in one second.
pub const MILLIS_PER_SECOND: f64 = 1000.0;

/// Valid memory categories across tools and learners.
pub const VALID_MEMORY_CATEGORIES: &[&str] = &["preference", "pattern", "fact", "interest"];

pub const MEMORY_CONTENT_MIN_LENGTH: usize = 5;
pub const MEMORY_CONTENT_MAX_LENGTH: usize = 1000;
pub const GOAL_CONTENT_MIN_LENGTH: usize = 5;
pub const GOAL_CONTENT_MAX_LENGTH: usize = 500;

pub const TOOL_LIMIT_MIN: i64 = 1;
pub const TOOL_LIMIT_MAX: i64 = 20;

pub const BROWSER_HISTORY_DEFAULT_DAYS: i64 = 7;
pub const BROWSER_HISTORY_MIN_DAYS: i64 = 1;
pub const BROWSER_HISTORY_MAX_DAYS: i64 = 365;
pub const BROWSER_HISTORY_DEFAULT_RESULTS: u64 = 50;
pub const BROWSER_HISTORY_MAX_RESULTS: u64 = 200;

pub const MICROS_PER_SECOND_I64: i64 = 1_000_000;

/// Chrome epoch offset: microseconds from 1601-01-01 to 1970-01-01.
pub const CHROME_EPOCH_OFFSET_US: i64 = 11_644_473_600_000_000;
/// Core Data epoch offset: seconds from 2001-01-01 to 1970-01-01.
pub const CORE_DATA_EPOCH_OFFSET_S: f64 = 978_307_200.0;
