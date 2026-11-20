use crate::calculation::PrayerTime;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const OFFICIAL_URL: &str = "https://muslim.uz/en/c/prayer";
const MAX_RESPONSE_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OfficialSchedule {
    pub date: NaiveDate,
    pub location: String,
    pub fetched_at: DateTime<Utc>,
    pub times: [i64; 6],
}

fn cache_path() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("salah/official.json")
}

fn parse_clock(value: &str) -> Option<i64> {
    let (hour, minute) = value.split_once(':')?;
    let hour: i64 = hour.parse().ok()?;
    let minute: i64 = minute.parse().ok()?;
    (hour < 24 && minute < 60).then_some(hour * 60 + minute)
}

fn visible_words(html: &str) -> Vec<String> {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                text.push(' ');
            }
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }
    text.replace("&nbsp;", " ")
        .replace("&#039;", "'")
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

pub fn parse_html(html: &[u8], date: NaiveDate) -> Result<OfficialSchedule, String> {
    if html.is_empty() || html.len() > MAX_RESPONSE_BYTES {
        return Err("official timetable response has an invalid size".into());
    }
    let html = std::str::from_utf8(html).map_err(|_| "official timetable is not UTF-8")?;
    let words = visible_words(html);
    let labels = ["Fajr", "Sunrise", "Dhuhr", "Asr", "Maghrib", "Isha"];
    let mut times = [0; 6];
    let mut cursor = 0;
    for (slot, label) in times.iter_mut().zip(labels) {
        let label_index = words[cursor..]
            .iter()
            .position(|word| word == label)
            .map(|index| cursor + index)
            .ok_or_else(|| format!("official timetable is missing {label}"))?;
        let (time_index, minutes) = words[label_index + 1..]
            .iter()
            .take(12)
            .enumerate()
            .find_map(|(index, word)| {
                parse_clock(word).map(|minutes| (label_index + 1 + index, minutes))
            })
            .ok_or_else(|| format!("official timetable has no time for {label}"))?;
        *slot = minutes;
        cursor = time_index + 1;
    }
    if !times.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err("official timetable times are not ordered".into());
    }
    Ok(OfficialSchedule {
        date,
        location: "Tashkent".into(),
        fetched_at: Utc::now(),
        times,
    })
}

pub fn fetch(date: NaiveDate) -> Result<OfficialSchedule, String> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--proto",
            "=https",
            "--tlsv1.2",
            "--max-redirs",
            "0",
            "--connect-timeout",
            "4",
            "--max-time",
            "8",
            "--max-filesize",
            &MAX_RESPONSE_BYTES.to_string(),
            OFFICIAL_URL,
        ])
        .output()
        .map_err(|e| format!("start curl: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "official timetable request failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_html(&output.stdout, date)
}

fn save_to(schedule: &OfficialSchedule, path: &Path) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = parent.join(format!(".official.{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let mut bytes = serde_json::to_vec_pretty(schedule).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(&temp, path).map_err(|e| e.to_string())
}

pub fn sync(date: NaiveDate) -> Result<OfficialSchedule, String> {
    let schedule = fetch(date)?;
    save_to(&schedule, &cache_path())?;
    Ok(schedule)
}

pub fn load(date: NaiveDate, location: &str) -> Option<OfficialSchedule> {
    if !location.to_ascii_lowercase().contains("tashkent") {
        return None;
    }
    let bytes = fs::read(cache_path()).ok()?;
    let schedule: OfficialSchedule = serde_json::from_slice(&bytes).ok()?;
    (schedule.date == date && schedule.location == "Tashkent").then_some(schedule)
}

pub fn prayer_times(schedule: &OfficialSchedule) -> Vec<PrayerTime> {
    [
        ("fajr", "Fajr"),
        ("sunrise", "Sunrise"),
        ("dhuhr", "Dhuhr"),
        ("asr", "Asr"),
        ("maghrib", "Maghrib"),
        ("isha", "Isha"),
    ]
    .into_iter()
    .zip(schedule.times)
    .map(|((id, name), minutes)| PrayerTime { id, name, minutes })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_board_page_shape() {
        let html = br#"<div>Tashkent city</div><b>Fajr</b><span>05:54</span><b>Sunrise</b>07:12
        <b>Dhuhr</b>12:14 <b>Asr</b>15:18 <b>Maghrib</b>17:03 <b>Isha</b>18:19"#;
        let result = parse_html(html, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()).unwrap();
        assert_eq!(result.times, [354, 432, 734, 918, 1023, 1099]);
    }
    #[test]
    fn rejects_missing_or_unordered_page() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        assert!(parse_html(b"not a timetable", date).is_err());
        assert!(
            parse_html(
                b"Fajr 10:00 Sunrise 09:00 Dhuhr 12:00 Asr 13:00 Maghrib 14:00 Isha 15:00",
                date
            )
            .is_err()
        );
    }
}
