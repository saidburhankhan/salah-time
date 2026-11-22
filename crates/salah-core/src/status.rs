use crate::config::Config;
use crate::planner::Today;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use salah_domain::day::{self, BlockKey, Boundary};
use salah_domain::error::Error;
use salah_prayer::schedule::{self, Schedule};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DisplayPrayer {
    pub id: &'static str,
    pub name: &'static str,
    pub time: String,
    pub passed: bool,
    pub is_next: bool,
}
#[derive(Debug, Serialize)]
pub struct NextPrayer {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
    pub time: String,
    pub date: String,
    pub remaining: String,
    pub remaining_short: String,
    /// UTC instant of the next boundary; clients derive countdowns from it.
    #[serde(skip_serializing)]
    pub at: DateTime<Utc>,
    pub remaining_seconds: i64,
}
#[derive(Debug, Serialize)]
pub struct DisplayBoundary {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
    pub time: String,
    pub passed: bool,
    pub is_next: bool,
}
#[derive(Debug, Serialize)]
pub struct CurrentBlock {
    /// Stable key of the block in progress, as stored with items.
    pub block: BlockKey,
    pub start: BoundaryPoint,
    pub end: BoundaryPoint,
    pub sleeping: bool,
}
#[derive(Debug, Serialize)]
pub struct BoundaryPoint {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
    pub time: String,
    pub date: String,
}
#[derive(Debug, Serialize)]
pub struct Status {
    pub schema_version: u32,
    pub location: String,
    pub method: String,
    pub method_name: &'static str,
    pub source: &'static str,
    pub source_name: &'static str,
    pub source_updated: Option<String>,
    pub date: String,
    /// The day (Fajr→Sleep) in progress; differs from `date` after midnight.
    pub day: String,
    pub sleep_time: String,
    pub label: String,
    pub next: NextPrayer,
    pub current_block: CurrentBlock,
    pub prayers: Vec<DisplayPrayer>,
    pub boundaries: Vec<DisplayBoundary>,
    /// Items for the widget. `None` with `planner_error` set when the store
    /// cannot be read, so prayer times still display.
    pub planner: Option<Today>,
    pub planner_error: Option<String>,
}

impl Status {
    pub fn attach_planner(&mut self, planner: Result<Today, Error>) {
        match planner {
            Ok(today) => {
                self.planner = Some(today);
                self.planner_error = None;
            }
            Err(error) => {
                self.planner = None;
                self.planner_error = Some(error.to_string());
            }
        }
    }
}

/// Version of the `status --json` shape. Bump it only for breaking changes;
/// new fields are additive and keep the version.
pub const SCHEMA_VERSION: u32 = 1;

pub fn build(config: &Config, utc_now: DateTime<Utc>) -> Result<Status, Error> {
    let schedule = schedule::resolve(config, utc_now)?;
    let next = &schedule.timeline.next;
    let now_seconds = schedule.now_seconds;
    let remaining_seconds = schedule.timeline.remaining_seconds;
    let remaining = format_duration(remaining_seconds, false);
    let remaining_short = format_duration(remaining_seconds, true);
    let rows: Vec<DisplayPrayer> = schedule
        .prayers
        .iter()
        .map(|p| DisplayPrayer {
            id: p.id,
            name: p.name,
            time: format_minutes(p.minutes),
            passed: p.minutes * 60 <= now_seconds,
            is_next: next.kind() == "prayer" && next.day_offset == 0 && p.id == next.id.as_str(),
        })
        .collect();
    let boundaries = display_boundaries(&rows, &config.sleep_time, next, now_seconds);
    let next_point = boundary_point(&schedule, next);
    let start_point = boundary_point(&schedule, &schedule.timeline.current_start);
    Ok(Status {
        schema_version: SCHEMA_VERSION,
        location: config.location.clone(),
        method: config.method.clone(),
        method_name: schedule.method_name,
        source: schedule.source.id,
        source_name: schedule.source.name,
        source_updated: schedule.source.updated.clone(),
        date: format_date(schedule.date),
        day: format_date(schedule.day()),
        sleep_time: config.sleep_time.clone(),
        label: format!("☾ {} {}", next.name(), remaining_short),
        next: NextPrayer {
            id: next.id.as_str(),
            name: next.name(),
            kind: next.kind(),
            time: format_minutes(next.minutes),
            date: next_point.date.clone(),
            remaining,
            remaining_short,
            at: schedule::boundary_at(config, schedule.date, next)?,
            remaining_seconds,
        },
        current_block: CurrentBlock {
            sleeping: schedule.timeline.sleeping(),
            block: schedule.current_block(),
            start: start_point,
            end: next_point,
        },
        prayers: rows,
        boundaries,
        planner: None,
        planner_error: None,
    })
}

/// Today's rows in chronological order: prayers, Sunrise, and the sleep
/// boundary that ends this evening (last even when it falls after midnight).
fn display_boundaries(
    prayers: &[DisplayPrayer],
    sleep_time: &str,
    next: &Boundary,
    now_seconds: i64,
) -> Vec<DisplayBoundary> {
    let mut boundaries: Vec<DisplayBoundary> = prayers
        .iter()
        .map(|prayer| DisplayBoundary {
            id: prayer.id,
            name: prayer.name,
            kind: if prayer.id == "sunrise" {
                "sunrise"
            } else {
                "prayer"
            },
            time: prayer.time.clone(),
            passed: prayer.passed,
            is_next: prayer.is_next,
        })
        .collect();
    let sleep_minutes = day::parse_clock_minutes(sleep_time)
        .expect("validated configuration has a valid sleep time");
    let fajr_minutes = prayers
        .iter()
        .find(|prayer| prayer.id == "fajr")
        .and_then(|prayer| day::parse_clock_minutes(&prayer.time).ok())
        .unwrap_or(0);
    let offset = day::evening_sleep_offset(sleep_minutes, fajr_minutes);
    let sleep_key = offset * 1440 + sleep_minutes;
    let position = boundaries
        .iter()
        .position(|row| day::parse_clock_minutes(&row.time).unwrap_or(i64::MAX) > sleep_key)
        .unwrap_or(boundaries.len());
    boundaries.insert(
        position,
        DisplayBoundary {
            id: "sleep",
            name: "Sleep",
            kind: "sleep",
            time: sleep_time.to_string(),
            passed: sleep_key * 60 <= now_seconds,
            // Highlight sleep whichever night it belongs to: the clock time is
            // the same and it is the only sleep row.
            is_next: next.kind() == "sleep",
        },
    );
    boundaries
}

fn boundary_point(schedule: &Schedule, boundary: &Boundary) -> BoundaryPoint {
    BoundaryPoint {
        id: boundary.id.as_str(),
        name: boundary.name(),
        kind: boundary.kind(),
        time: format_minutes(boundary.minutes),
        date: format_date(schedule.occurs_on(boundary)),
    }
}

pub fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn format_minutes(minutes: i64) -> String {
    format!(
        "{:02}:{:02}",
        minutes.div_euclid(60),
        minutes.rem_euclid(60)
    )
}
pub fn format_duration(seconds: i64, short: bool) -> String {
    let seconds = seconds.max(0);
    let duration = Duration::seconds(seconds);
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;
    if short {
        if hours > 0 {
            format!("{hours}h {minutes:02}m")
        } else {
            format!("{minutes}m")
        }
    } else {
        format!("{hours:02}:{minutes:02}:{:02}", seconds % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    #[test]
    fn status_selects_next_prayer() {
        let report = build(
            &Config::default(),
            Utc.with_ymd_and_hms(2026, 11, 20, 7, 0, 0).unwrap(),
        )
        .unwrap();
        assert_eq!(report.next.id, "dhuhr");
        assert!(report.next.remaining_seconds > 0);
    }
    #[test]
    fn duration_is_bar_friendly() {
        assert_eq!(format_duration(5460, true), "1h 31m");
        assert_eq!(format_duration(540, true), "9m");
    }

    #[test]
    fn after_isha_points_to_sleep_without_highlighting_a_prayer() {
        let report = build(
            &Config::default(),
            // 14:00 UTC is 19:00 in Tashkent, after Isha and before sleep.
            Utc.with_ymd_and_hms(2026, 11, 20, 14, 0, 0).unwrap(),
        )
        .unwrap();
        assert_eq!(report.next.id, "sleep");
        assert_eq!(report.next.kind, "sleep");
        assert!(report.next.remaining_seconds > 0);
        assert!(report.prayers.iter().all(|prayer| !prayer.is_next));
        assert_eq!(report.current_block.start.id, "isha");
        assert!(!report.current_block.sleeping);
        assert_eq!(report.boundaries.last().unwrap().id, "sleep");
        assert!(report.boundaries.last().unwrap().is_next);
    }

    #[test]
    fn post_midnight_bedtime_is_next_after_isha_and_listed_last() {
        let config = Config {
            sleep_time: "00:30".into(),
            ..Config::default()
        };
        // 14:00 UTC is 19:00 in Tashkent, after Isha.
        let report = build(
            &config,
            Utc.with_ymd_and_hms(2026, 11, 20, 14, 0, 0).unwrap(),
        )
        .unwrap();
        assert_eq!(report.next.id, "sleep");
        assert_eq!(report.next.date, "2026-11-21");
        assert_eq!(report.next.remaining_seconds, 5 * 3600 + 30 * 60);
        assert!(!report.current_block.sleeping);
        let last = report.boundaries.last().unwrap();
        assert_eq!(last.id, "sleep");
        assert!(last.is_next && !last.passed);
        assert!(report.boundaries.iter().any(|row| row.kind == "sunrise"));
    }

    #[test]
    fn after_sleep_is_marked_sleeping_until_tomorrows_fajr() {
        let report = build(
            &Config::default(),
            // 18:30 UTC is 23:30 in Tashkent, after the default sleep boundary.
            Utc.with_ymd_and_hms(2026, 11, 20, 18, 30, 0).unwrap(),
        )
        .unwrap();
        assert_eq!(report.next.id, "fajr");
        assert_eq!(report.next.date, "2026-11-21");
        assert!(report.current_block.sleeping);
        assert_eq!(report.current_block.start.id, "sleep");
    }

    #[test]
    fn json_contract_contains_fields_consumed_by_the_plugin() {
        let report = build(
            &Config::default(),
            Utc.with_ymd_and_hms(2026, 11, 20, 7, 0, 0).unwrap(),
        )
        .unwrap();
        let json = serde_json::to_value(report).unwrap();
        assert!(json["location"].is_string());
        assert!(json["method"].is_string());
        assert!(json["next"]["name"].is_string());
        assert!(json["next"]["time"].is_string());
        assert!(json["next"]["remaining_seconds"].is_i64());
        assert!(json["next"]["kind"].is_string());
        assert!(json["current_block"]["sleeping"].is_boolean());
        assert!(json["boundaries"].is_array());
        assert!(json["prayers"].is_array());
        for prayer in json["prayers"].as_array().unwrap() {
            assert!(prayer["name"].is_string());
            assert!(prayer["time"].is_string());
            assert!(prayer["passed"].is_boolean());
            assert!(prayer["is_next"].is_boolean());
        }
    }
}
