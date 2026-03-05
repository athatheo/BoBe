pub(crate) const DEFAULT_SOUL_FALLBACK: &str = "You are BoBe, a helpful AI assistant.";

pub(crate) const MILLIS_PER_SECOND: f64 = 1000.0;

pub(crate) const VALID_MEMORY_CATEGORIES: &[&str] = &["preference", "pattern", "fact", "interest"];

pub(crate) const MEMORY_CONTENT_MIN_LENGTH: usize = 5;
pub(crate) const MEMORY_CONTENT_MAX_LENGTH: usize = 1000;
pub(crate) const GOAL_CONTENT_MIN_LENGTH: usize = 5;
pub(crate) const GOAL_CONTENT_MAX_LENGTH: usize = 500;

pub(crate) const TOOL_LIMIT_MIN: i64 = 1;
pub(crate) const TOOL_LIMIT_MAX: i64 = 20;

pub(crate) const BROWSER_HISTORY_DEFAULT_DAYS: i64 = 7;
pub(crate) const BROWSER_HISTORY_MIN_DAYS: i64 = 1;
pub(crate) const BROWSER_HISTORY_MAX_DAYS: i64 = 365;
pub(crate) const BROWSER_HISTORY_DEFAULT_RESULTS: u64 = 50;
pub(crate) const BROWSER_HISTORY_MAX_RESULTS: u64 = 200;

pub(crate) const MICROS_PER_SECOND_I64: i64 = 1_000_000;

/// Chrome epoch offset: microseconds from 1601-01-01 to 1970-01-01.
pub(crate) const CHROME_EPOCH_OFFSET_US: i64 = 11_644_473_600_000_000;
/// Core Data epoch offset: seconds from 2001-01-01 to 1970-01-01.
pub(crate) const CORE_DATA_EPOCH_OFFSET_S: f64 = 978_307_200.0;
