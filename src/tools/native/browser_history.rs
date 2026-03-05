use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt::Write;
use std::path::PathBuf;

use super::base::NativeTool;
use crate::constants::{
    BROWSER_HISTORY_DEFAULT_DAYS, BROWSER_HISTORY_DEFAULT_RESULTS, BROWSER_HISTORY_MAX_DAYS,
    BROWSER_HISTORY_MAX_RESULTS, BROWSER_HISTORY_MIN_DAYS, CHROME_EPOCH_OFFSET_US,
    CORE_DATA_EPOCH_OFFSET_S, MICROS_PER_SECOND_I64,
};
use crate::error::AppError;
use crate::tools::ToolExecutionContext;

pub(crate) struct BrowserHistoryTool;

impl Default for BrowserHistoryTool {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserHistoryTool {
    pub(crate) fn new() -> Self {
        Self
    }

    fn chrome_history_path() -> Option<PathBuf> {
        dirs::home_dir()
            .map(|h| h.join("Library/Application Support/Google/Chrome/Default/History"))
    }

    fn safari_history_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join("Library/Safari/History.db"))
    }

    fn firefox_profile_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join("Library/Application Support/Firefox/Profiles"))
    }
}

#[async_trait]
impl NativeTool for BrowserHistoryTool {
    fn name(&self) -> &str {
        "browser_history"
    }

    fn description(&self) -> &str {
        "Search browser history across Chrome, Safari, and Firefox. macOS only."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search term to match against URLs and page titles"
                },
                "days": {
                    "type": "integer",
                    "description": "Number of days to search back (default: 7, max: 365)",
                    "default": 7
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum results to return (default: 50)",
                    "default": 50
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        arguments: HashMap<String, Value>,
        _context: Option<&ToolExecutionContext>,
    ) -> Result<String, AppError> {
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("'query' is required".into()))?
            .to_owned();

        let days = arguments
            .get("days")
            .and_then(Value::as_i64)
            .unwrap_or(BROWSER_HISTORY_DEFAULT_DAYS)
            .clamp(BROWSER_HISTORY_MIN_DAYS, BROWSER_HISTORY_MAX_DAYS);

        let max_results = arguments
            .get("max_results")
            .and_then(Value::as_u64)
            .unwrap_or(BROWSER_HISTORY_DEFAULT_RESULTS)
            .min(BROWSER_HISTORY_MAX_RESULTS) as usize;

        let mut all_results: Vec<(String, String, String)> = Vec::new();

        // Search Chrome
        if let Some(chrome_path) = Self::chrome_history_path()
            && chrome_path.exists()
            && let Ok(r) = search_chrome(&chrome_path, &query, days, max_results).await
        {
            all_results.extend(r);
        }

        // Search Safari
        if let Some(safari_path) = Self::safari_history_path()
            && safari_path.exists()
            && let Ok(r) = search_safari(&safari_path, &query, days, max_results).await
        {
            all_results.extend(r);
        }

        // Search Firefox
        if let Some(firefox_dir) = Self::firefox_profile_dir()
            && firefox_dir.exists()
            && let Ok(r) = search_firefox(&firefox_dir, &query, days, max_results).await
        {
            all_results.extend(r);
        }

        // Sort by timestamp (newest first) and limit
        all_results.sort_by(|a, b| b.2.cmp(&a.2));
        all_results.truncate(max_results);

        if all_results.is_empty() {
            return Ok(format!("No browser history matches for '{query}'."));
        }

        let mut output = format!(
            "Found {} history entries for '{query}':\n\n",
            all_results.len()
        );
        for (title, url, timestamp) in &all_results {
            let display_title = if title.is_empty() {
                "(no title)"
            } else {
                title
            };
            let _ = write!(output, "• {display_title}\n  {url}\n  {timestamp}\n\n");
        }
        Ok(output)
    }
}

/// The resulting SQL must include `ESCAPE '\'` for the wildcard escaping to work.
fn escape_sql_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\'' => out.push_str("''"),
            '%' => out.push_str("\\%"),
            '_' => out.push_str("\\_"),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}

/// Copies the DB to a temp file to avoid locking the browser's DB.
async fn query_sqlite(db_path: &PathBuf, temp_name: &str, sql: &str) -> Result<String, AppError> {
    let temp = std::env::temp_dir().join(temp_name);
    tokio::fs::copy(db_path, &temp)
        .await
        .map_err(|e| AppError::Tool(format!("Cannot copy DB: {e}")))?;

    let output = tokio::process::Command::new("sqlite3")
        .arg("-separator")
        .arg("\t")
        .arg(&temp)
        .arg(sql)
        .output()
        .await
        .map_err(|e| AppError::Tool(format!("sqlite3 command failed: {e}")))?;

    let _ = tokio::fs::remove_file(&temp).await;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Tool(format!("sqlite3 error: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_rows(output: &str) -> Vec<(String, String, String)> {
    output
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() >= 2 {
                Some((
                    parts.first().unwrap_or(&"").to_string(),
                    parts.get(1).unwrap_or(&"").to_string(),
                    parts.get(2).unwrap_or(&"").to_string(),
                ))
            } else {
                None
            }
        })
        .collect()
}

async fn search_chrome(
    db_path: &PathBuf,
    query: &str,
    days: i64,
    max_results: usize,
) -> Result<Vec<(String, String, String)>, AppError> {
    let cutoff_us = (chrono::Utc::now() - chrono::Duration::days(days)).timestamp()
        * MICROS_PER_SECOND_I64
        + CHROME_EPOCH_OFFSET_US;

    let escaped = escape_sql_like(query);
    let sql = format!(
        "SELECT COALESCE(u.title, ''), u.url, \
         datetime((v.visit_time - {CHROME_EPOCH_OFFSET_US}) / {MICROS_PER_SECOND_I64}, 'unixepoch') \
         FROM visits v JOIN urls u ON v.url = u.id \
         WHERE (u.url LIKE '%{escaped}%' ESCAPE '\\' OR u.title LIKE '%{escaped}%' ESCAPE '\\') \
         AND v.visit_time > {cutoff_us} \
         ORDER BY v.visit_time DESC LIMIT {max_results};"
    );

    let output = query_sqlite(db_path, "bobe_chrome_hist", &sql).await?;
    Ok(parse_rows(&output))
}

async fn search_safari(
    db_path: &PathBuf,
    query: &str,
    days: i64,
    max_results: usize,
) -> Result<Vec<(String, String, String)>, AppError> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).timestamp() as f64
        - CORE_DATA_EPOCH_OFFSET_S;

    let escaped = escape_sql_like(query);
    let sql = format!(
        "SELECT COALESCE(hv.title, ''), hi.url, \
         datetime(hv.visit_time + {CORE_DATA_EPOCH_OFFSET_S}, 'unixepoch') \
         FROM history_visits hv JOIN history_items hi ON hv.history_item = hi.id \
         WHERE (hi.url LIKE '%{escaped}%' ESCAPE '\\' OR hv.title LIKE '%{escaped}%' ESCAPE '\\') \
         AND hv.visit_time > {cutoff} \
         ORDER BY hv.visit_time DESC LIMIT {max_results};"
    );

    let output = query_sqlite(db_path, "bobe_safari_hist", &sql).await?;
    Ok(parse_rows(&output))
}

async fn search_firefox(
    profiles_dir: &PathBuf,
    query: &str,
    days: i64,
    max_results: usize,
) -> Result<Vec<(String, String, String)>, AppError> {
    // Find first profile with places.sqlite
    let mut read_dir = tokio::fs::read_dir(profiles_dir)
        .await
        .map_err(|e| AppError::Tool(format!("Cannot read Firefox profiles: {e}")))?;

    let mut db_path = None;
    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| AppError::Tool(format!("Error reading profiles: {e}")))?
    {
        let places = entry.path().join("places.sqlite");
        if places.exists() {
            db_path = Some(places);
            break;
        }
    }

    let Some(db_path) = db_path else {
        return Ok(Vec::new());
    };

    let cutoff_us =
        (chrono::Utc::now() - chrono::Duration::days(days)).timestamp() * MICROS_PER_SECOND_I64;

    let escaped = escape_sql_like(query);
    let sql = format!(
        "SELECT COALESCE(p.title, ''), p.url, \
         datetime(v.visit_date / {MICROS_PER_SECOND_I64}, 'unixepoch') \
         FROM moz_historyvisits v JOIN moz_places p ON v.place_id = p.id \
         WHERE (p.url LIKE '%{escaped}%' ESCAPE '\\' OR p.title LIKE '%{escaped}%' ESCAPE '\\') \
         AND v.visit_date > {cutoff_us} \
         ORDER BY v.visit_date DESC LIMIT {max_results};"
    );

    let output = query_sqlite(&db_path, "bobe_firefox_hist", &sql).await?;
    Ok(parse_rows(&output))
}
