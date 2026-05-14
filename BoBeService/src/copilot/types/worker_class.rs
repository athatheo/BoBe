//! Worker pool taxonomy. Five classes share the Copilot SDK session
//! infrastructure but with different timeouts + run modes:
//!   - Chat: interactive, user-facing, hot-path
//!   - Goals/Decide/Vision/Consolidate: autopilot batch jobs

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WorkerClass {
    Goals,
    Vision,
    Chat,
    Consolidate,
    Decide,
}

impl WorkerClass {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            WorkerClass::Goals => "goals",
            WorkerClass::Vision => "vision",
            WorkerClass::Chat => "chat",
            WorkerClass::Consolidate => "consolidate",
            WorkerClass::Decide => "decide",
        }
    }

    pub(crate) const fn turn_timeout(self) -> Duration {
        match self {
            WorkerClass::Goals => Duration::from_mins(3),
            WorkerClass::Vision => Duration::from_mins(5),
            WorkerClass::Chat => Duration::from_mins(2),
            WorkerClass::Consolidate => Duration::from_mins(15),
            // Decide is hot-path: every screen capture waits on it.
            WorkerClass::Decide => Duration::from_secs(45),
        }
    }

    pub(crate) const fn mode(self) -> &'static str {
        match self {
            WorkerClass::Chat => "interactive",
            _ => "autopilot",
        }
    }
}
