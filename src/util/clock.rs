use chrono::{DateTime, Utc};

/// Clock abstraction for testability.
#[allow(dead_code)]
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Real system clock.
#[allow(dead_code)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
