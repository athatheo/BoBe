//! Worker pool taxonomy. Five classes share the Copilot SDK session
//! infrastructure but with different timeouts + run modes:
//!   - Chat: interactive, user-facing, hot-path
//!   - Goals/Vision/Consolidate: autopilot batch jobs

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WorkerClass {
    Goals,
    Vision,
    Chat,
    Consolidate,
}

impl WorkerClass {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            WorkerClass::Goals => "goals",
            WorkerClass::Vision => "vision",
            WorkerClass::Chat => "chat",
            WorkerClass::Consolidate => "consolidate",
        }
    }

    pub(crate) const fn turn_timeout(self) -> Duration {
        match self {
            WorkerClass::Goals => Duration::from_mins(3),
            WorkerClass::Vision => Duration::from_mins(5),
            WorkerClass::Chat => Duration::from_mins(2),
            WorkerClass::Consolidate => Duration::from_mins(15),
        }
    }

    pub(crate) const fn mode(self) -> &'static str {
        match self {
            WorkerClass::Chat => "interactive",
            _ => "autopilot",
        }
    }

    /// Runaway guard for autonomous sessions. Values are deliberately above
    /// normal daily use; they stop a broken trigger loop without constraining
    /// interactive chat, which remains user-controlled and unlimited here.
    pub(crate) const fn max_ai_credits(self) -> Option<f64> {
        match self {
            WorkerClass::Chat => None,
            WorkerClass::Vision | WorkerClass::Goals => Some(50.0),
            WorkerClass::Consolidate => Some(20.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autonomous_workers_are_bounded_but_chat_is_not() {
        assert_eq!(WorkerClass::Chat.max_ai_credits(), None);
        assert!(WorkerClass::Vision.max_ai_credits().is_some());
        assert!(WorkerClass::Goals.max_ai_credits().is_some());
        assert!(WorkerClass::Consolidate.max_ai_credits().is_some());
    }
}
