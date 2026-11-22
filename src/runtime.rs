//! Injectable runtime capabilities.
//!
//! Library code receives a `DateTime<Utc>` and an [`IdGenerator`] from its
//! caller. Only the CLI reads the system clock and constructs the real
//! implementations; tests can use [`FixedClock`] and [`SequenceIds`] to make
//! behaviour deterministic.

use chrono::{DateTime, Utc};
use std::sync::atomic::{AtomicU64, Ordering};

/// Source of the current instant.
pub trait Clock {
    fn now(&self) -> DateTime<Utc>;
}

/// Source of new entity ids.
pub trait IdGenerator {
    fn new_id(&self) -> String;
}

/// Reads the system clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Always returns the same instant; useful in tests.
#[derive(Clone, Copy, Debug)]
pub struct FixedClock {
    now: DateTime<Utc>,
}

impl FixedClock {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self { now }
    }
}

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.now
    }
}

/// Generates random UUID v4 ids.
#[derive(Clone, Copy, Debug, Default)]
pub struct UuidV4;

impl IdGenerator for UuidV4 {
    fn new_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

/// Generates deterministic UUID-shaped ids from a counter; useful in tests.
#[derive(Debug)]
pub struct SequenceIds {
    next: AtomicU64,
}

impl SequenceIds {
    pub const fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }
}

impl Default for SequenceIds {
    fn default() -> Self {
        Self::new()
    }
}

impl IdGenerator for SequenceIds {
    fn new_id(&self) -> String {
        let value = self.next.fetch_add(1, Ordering::Relaxed);
        format!("00000000-0000-4000-8000-{value:012x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn fixed_clock_always_returns_the_same_instant() {
        let instant = Utc.with_ymd_and_hms(2031, 5, 14, 4, 30, 0).unwrap();
        let clock = FixedClock::new(instant);
        assert_eq!(clock.now(), instant);
        assert_eq!(clock.now(), instant);
    }

    #[test]
    fn sequence_ids_are_unique_and_deterministic() {
        let ids = SequenceIds::new();
        let first = ids.new_id();
        let second = ids.new_id();
        assert_eq!(first, "00000000-0000-4000-8000-000000000001");
        assert_eq!(second, "00000000-0000-4000-8000-000000000002");
        assert_ne!(first, second);
    }
}
