//! `UsageMeter` — aggregates `assistant.usage` events into per-class
//! counters. Surfaces the data BoBe needs for "X premium requests today,
//! Y left" UX and for cost observability.

#![allow(
    dead_code,
    reason = "Phase 6: snapshot/snapshot_all consumed by Phase 5 /api/health migration"
)]
//!
//! Wire format (from the SDK's streaming events doc):
//!
//! ```json
//! {
//!   "model": "gpt-4.1",
//!   "inputTokens": 1234,
//!   "outputTokens": 567,
//!   "cacheReadTokens": 89,
//!   "cacheWriteTokens": 0,
//!   "cost": 1.0,
//!   "duration": 2400,
//!   "providerCallId": "req_..."
//! }
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::types::{UsageSnapshot, WorkerClass};

pub(crate) struct UsageMeter {
    inner: RwLock<HashMap<WorkerClass, UsageSnapshot>>,
}

impl UsageMeter {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: RwLock::new(HashMap::new()),
        })
    }

    /// Record one `assistant.usage` event for `class`. Tolerates missing
    /// fields — Copilot may omit `cost` for sub-agent calls, etc.
    pub(crate) fn record(&self, class: WorkerClass, data: &serde_json::Value) {
        let mut guard = match self.inner.write() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let entry = guard.entry(class).or_default();
        entry.api_calls += 1;
        if let Some(t) = data.get("inputTokens").and_then(serde_json::Value::as_u64) {
            entry.input_tokens += t;
        }
        if let Some(t) = data.get("outputTokens").and_then(serde_json::Value::as_u64) {
            entry.output_tokens += t;
        }
        if let Some(t) = data.get("cacheReadTokens").and_then(serde_json::Value::as_u64) {
            entry.cache_read_tokens += t;
        }
        if let Some(t) = data.get("cacheWriteTokens").and_then(serde_json::Value::as_u64) {
            entry.cache_write_tokens += t;
        }
        if let Some(c) = data.get("cost").and_then(serde_json::Value::as_f64) {
            entry.cost_units += c;
        }
    }

    pub(crate) fn snapshot(&self, class: WorkerClass) -> UsageSnapshot {
        let guard = match self.inner.read() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.get(&class).cloned().unwrap_or_default()
    }

    /// All-classes snapshot — useful for `/api/health` style endpoints.
    pub(crate) fn snapshot_all(&self) -> HashMap<WorkerClass, UsageSnapshot> {
        let guard = match self.inner.read() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.clone()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn record_accumulates_across_calls() {
        let meter = UsageMeter::new();
        meter.record(
            WorkerClass::Goals,
            &json!({
                "inputTokens": 100,
                "outputTokens": 50,
                "cost": 1.0,
            }),
        );
        meter.record(
            WorkerClass::Goals,
            &json!({
                "inputTokens": 200,
                "outputTokens": 75,
                "cacheReadTokens": 25,
                "cost": 2.0,
            }),
        );

        let s = meter.snapshot(WorkerClass::Goals);
        assert_eq!(s.api_calls, 2);
        assert_eq!(s.input_tokens, 300);
        assert_eq!(s.output_tokens, 125);
        assert_eq!(s.cache_read_tokens, 25);
        assert!((s.cost_units - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn snapshot_per_class_isolated() {
        let meter = UsageMeter::new();
        meter.record(WorkerClass::Goals, &json!({"inputTokens": 10}));
        meter.record(WorkerClass::Chat, &json!({"inputTokens": 20}));
        assert_eq!(meter.snapshot(WorkerClass::Goals).input_tokens, 10);
        assert_eq!(meter.snapshot(WorkerClass::Chat).input_tokens, 20);
        assert_eq!(meter.snapshot(WorkerClass::Vision).input_tokens, 0);
    }

    #[test]
    fn record_tolerates_missing_fields() {
        let meter = UsageMeter::new();
        meter.record(WorkerClass::Goals, &json!({}));
        let s = meter.snapshot(WorkerClass::Goals);
        assert_eq!(s.api_calls, 1);
        assert_eq!(s.input_tokens, 0);
        assert!((s.cost_units - 0.0).abs() < f64::EPSILON);
    }
}
