//! Checkin scheduler — schedules periodic check-ins at configured times.
//!
//! Supports scheduled times (e.g. "09:00") and/or interval-based (every N minutes).
//! Adds random jitter to feel more natural.

use chrono::{DateTime, Duration, Local, NaiveTime, Utc};
use rand::RngExt;
use tracing::{debug, info, warn};

pub struct CheckinScheduler {
    times: Vec<NaiveTime>,
    interval_minutes: Option<u64>,
    jitter_minutes: i32,
    enabled: bool,
    last_checkin: Option<DateTime<Utc>>,
    next_checkin: Option<DateTime<Utc>>,
    next_interval_checkin: Option<DateTime<Utc>>,
}

impl CheckinScheduler {
    pub fn new(
        times: &[String],
        interval_minutes: Option<u64>,
        jitter_minutes: u32,
        enabled: bool,
    ) -> Self {
        let parsed_times = Self::parse_times(times);
        info!(
            times = ?parsed_times.iter().map(|t| t.format("%H:%M").to_string()).collect::<Vec<_>>(),
            interval_minutes = ?interval_minutes,
            jitter_minutes,
            enabled,
            "checkin_scheduler.initialized"
        );
        Self {
            times: parsed_times,
            interval_minutes,
            jitter_minutes: jitter_minutes as i32,
            enabled,
            last_checkin: None,
            next_checkin: None,
            next_interval_checkin: None,
        }
    }

    fn parse_times(times: &[String]) -> Vec<NaiveTime> {
        let mut parsed = Vec::new();
        for time_str in times {
            let parts: Vec<&str> = time_str.split(':').collect();
            if parts.len() == 2 {
                if let (Ok(hour), Ok(minute)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                    && let Some(t) = NaiveTime::from_hms_opt(hour, minute, 0)
                {
                    parsed.push(t);
                }
            } else {
                warn!(time = %time_str, "checkin_scheduler.invalid_time_format");
            }
        }
        parsed.sort();
        parsed
    }

    pub fn should_checkin(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        if self.times.is_empty() && self.interval_minutes.is_none() {
            return false;
        }

        let now = Utc::now();

        // Check scheduled times
        if !self.times.is_empty() {
            if self.next_checkin.is_none() {
                self.schedule_next_checkin(now);
            }
            if let Some(next) = self.next_checkin
                && now >= next
                && self.last_checkin.is_none_or(|last| last < next)
            {
                info!(
                    trigger_type = "scheduled_time",
                    scheduled = %next,
                    "checkin_scheduler.triggered"
                );
                return true;
            }
        }

        // Check interval-based
        if let Some(_interval) = self.interval_minutes {
            if self.next_interval_checkin.is_none() {
                self.schedule_next_interval(now);
            }
            if let Some(next) = self.next_interval_checkin
                && now >= next
            {
                info!(
                    trigger_type = "interval",
                    scheduled = %next,
                    "checkin_scheduler.triggered"
                );
                return true;
            }
        }

        false
    }

    pub fn mark_checkin_done(&mut self) {
        let now = Utc::now();
        self.last_checkin = Some(now);

        if !self.times.is_empty() {
            self.schedule_next_checkin(now);
        }
        if self.interval_minutes.is_some() {
            self.schedule_next_interval(now);
        }

        let next = self.get_next_checkin_time();
        info!(
            next = ?next.map(|t| t.format("%H:%M:%S").to_string()),
            "checkin_scheduler.done"
        );
    }

    pub fn get_next_checkin_time(&mut self) -> Option<DateTime<Utc>> {
        if !self.enabled {
            return None;
        }
        if self.times.is_empty() && self.interval_minutes.is_none() {
            return None;
        }

        let now = Utc::now();
        let mut candidates = Vec::new();

        if !self.times.is_empty() {
            if self.next_checkin.is_none() {
                self.schedule_next_checkin(now);
            }
            if let Some(next) = self.next_checkin {
                candidates.push(next);
            }
        }

        if self.interval_minutes.is_some() {
            if self.next_interval_checkin.is_none() {
                self.schedule_next_interval(now);
            }
            if let Some(next) = self.next_interval_checkin {
                candidates.push(next);
            }
        }

        candidates.into_iter().min()
    }

    fn schedule_next_checkin(&mut self, after: DateTime<Utc>) {
        if self.times.is_empty() {
            self.next_checkin = None;
            return;
        }

        let local_now = after.with_timezone(&Local);
        let today = local_now.date_naive();
        let local_time = local_now.time();

        // Find next scheduled time today
        let mut next_time: Option<DateTime<Utc>> = None;
        for &t in &self.times {
            if t > local_time {
                let naive_dt = today.and_time(t);
                let local_dt = naive_dt.and_local_timezone(Local).single();
                if let Some(dt) = local_dt {
                    next_time = Some(dt.with_timezone(&Utc));
                    break;
                }
            }
        }

        // If none today, use first time tomorrow
        if next_time.is_none() {
            let tomorrow = today + chrono::Duration::days(1);
            let naive_dt = tomorrow.and_time(self.times[0]);
            if let Some(dt) = naive_dt.and_local_timezone(Local).single() {
                next_time = Some(dt.with_timezone(&Utc));
            }
        }

        // Add jitter
        if let Some(ref mut t) = next_time
            && self.jitter_minutes > 0
        {
            let jitter = rand::rng().random_range(-self.jitter_minutes..=self.jitter_minutes);
            *t += Duration::minutes(jitter as i64);
        }

        self.next_checkin = next_time;
        if let Some(t) = next_time {
            debug!(next = %t.format("%Y-%m-%d %H:%M:%S"), "checkin_scheduler.scheduled_next_time");
        }
    }

    fn schedule_next_interval(&mut self, after: DateTime<Utc>) {
        let Some(interval) = self.interval_minutes else {
            self.next_interval_checkin = None;
            return;
        };

        let mut next_time = after + Duration::minutes(interval as i64);

        if self.jitter_minutes > 0 {
            let jitter = rand::rng().random_range(-self.jitter_minutes..=self.jitter_minutes);
            next_time += Duration::minutes(jitter as i64);
        }

        self.next_interval_checkin = Some(next_time);
        debug!(next = %next_time.format("%Y-%m-%d %H:%M:%S"), "checkin_scheduler.scheduled_interval");
    }
}
