use crate::calculation::{PrayerTime, calculate, method};
use crate::{config::Config, official};
use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate, Timelike, Utc};
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
    pub time: String,
    pub remaining: String,
    pub remaining_short: String,
    pub remaining_seconds: i64,
}
#[derive(Debug, Serialize)]
pub struct Status {
    pub location: String,
    pub method: String,
    pub method_name: &'static str,
    pub source: &'static str,
    pub source_name: &'static str,
    pub source_updated: Option<String>,
    pub date: String,
    pub label: String,
    pub next: NextPrayer,
    pub prayers: Vec<DisplayPrayer>,
}

pub fn build(config: &Config, utc_now: DateTime<Utc>) -> Result<Status, String> {
    let selected =
        method(&config.method).ok_or_else(|| format!("unknown method: {}", config.method))?;
    let offset = FixedOffset::east_opt((config.timezone_offset * 3600.0).round() as i32)
        .ok_or("invalid timezone offset")?;
    let now = utc_now.with_timezone(&offset);
    let date = now.date_naive();
    let calculated_today = calculate(
        date,
        config.latitude,
        config.longitude,
        config.timezone_offset,
        selected,
        config.asr == "hanafi",
    );
    let official_schedule = if config.method == "uzbekistan" {
        official::load(date, &config.location)
    } else {
        None
    };
    let (today, source, source_name, source_updated) = match official_schedule {
        Some(schedule) => {
            let updated = Some(schedule.fetched_at.to_rfc3339());
            (
                official::prayer_times(&schedule),
                "official",
                "Muslim Board of Uzbekistan",
                updated,
            )
        }
        None => (calculated_today, "calculated", selected.name, None),
    };
    let now_seconds = now.hour() as i64 * 3600 + now.minute() as i64 * 60 + now.second() as i64;
    if let Some(next) = today
        .iter()
        .find(|p| p.id != "sunrise" && p.minutes * 60 > now_seconds)
    {
        return Ok(finish(
            config,
            selected.name,
            date,
            today.clone(),
            next.clone(),
            next.minutes * 60 - now_seconds,
            now_seconds,
            true,
            source,
            source_name,
            source_updated,
        ));
    }
    let tomorrow = date.succ_opt().ok_or("date overflow")?;
    let next = calculate(
        tomorrow,
        config.latitude,
        config.longitude,
        config.timezone_offset,
        selected,
        config.asr == "hanafi",
    )
    .into_iter()
    .find(|p| p.id == "fajr")
    .ok_or("missing fajr")?;
    Ok(finish(
        config,
        selected.name,
        date,
        today,
        next.clone(),
        86_400 + next.minutes * 60 - now_seconds,
        now_seconds,
        false,
        source,
        source_name,
        source_updated,
    ))
}

// These values are the complete boundary between schedule selection and JSON
// presentation; keeping them explicit makes tomorrow-Fajr handling auditable.
#[allow(clippy::too_many_arguments)]
fn finish(
    config: &Config,
    method_name: &'static str,
    date: NaiveDate,
    prayers: Vec<PrayerTime>,
    next: PrayerTime,
    remaining_seconds: i64,
    now_seconds: i64,
    next_is_on_display_date: bool,
    source: &'static str,
    source_name: &'static str,
    source_updated: Option<String>,
) -> Status {
    let remaining = format_duration(remaining_seconds, false);
    let remaining_short = format_duration(remaining_seconds, true);
    let rows = prayers
        .into_iter()
        .map(|p| DisplayPrayer {
            id: p.id,
            name: p.name,
            time: format_minutes(p.minutes),
            passed: p.minutes * 60 <= now_seconds,
            // After Isha, `next` is tomorrow's Fajr while the rows still
            // describe today. Do not highlight today's already-passed Fajr.
            is_next: next_is_on_display_date && p.id == next.id,
        })
        .collect();
    Status {
        location: config.location.clone(),
        method: config.method.clone(),
        method_name,
        source,
        source_name,
        source_updated,
        date: format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day()),
        label: format!("☾ {} {}", next.name, remaining_short),
        next: NextPrayer {
            id: next.id,
            name: next.name,
            time: format_minutes(next.minutes),
            remaining,
            remaining_short,
            remaining_seconds,
        },
        prayers: rows,
    }
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
    fn after_isha_points_to_tomorrow_without_highlighting_today() {
        let report = build(
            &Config::default(),
            // 14:00 UTC is 19:00 in Tashkent, after Isha on this date.
            Utc.with_ymd_and_hms(2026, 11, 20, 14, 0, 0).unwrap(),
        )
        .unwrap();
        assert_eq!(report.next.id, "fajr");
        assert!(report.next.remaining_seconds > 0);
        assert!(report.next.remaining_seconds < 86_400);
        assert!(report.prayers.iter().all(|prayer| !prayer.is_next));
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
        assert!(json["prayers"].is_array());
        for prayer in json["prayers"].as_array().unwrap() {
            assert!(prayer["name"].is_string());
            assert!(prayer["time"].is_string());
            assert!(prayer["passed"].is_boolean());
            assert!(prayer["is_next"].is_boolean());
        }
    }
}
