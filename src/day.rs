use crate::calculation::PrayerTime;

#[derive(Clone, Debug, PartialEq)]
pub struct Boundary {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
    pub minutes: i64,
    pub day_offset: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    pub next: Boundary,
    pub current_start: Boundary,
    pub remaining_seconds: i64,
}

pub fn parse_clock_minutes(value: &str) -> Result<i64, String> {
    let bytes = value.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || !bytes[0].is_ascii_digit()
        || !bytes[1].is_ascii_digit()
        || !bytes[3].is_ascii_digit()
        || !bytes[4].is_ascii_digit()
    {
        return Err("time must use HH:MM".into());
    }
    let hours = i64::from(bytes[0] - b'0') * 10 + i64::from(bytes[1] - b'0');
    let minutes = i64::from(bytes[3] - b'0') * 10 + i64::from(bytes[4] - b'0');
    if hours > 23 || minutes > 59 {
        return Err("time is outside the 24-hour clock".into());
    }
    Ok(hours * 60 + minutes)
}

fn prayer_boundary(prayer: &PrayerTime, day_offset: i64) -> Boundary {
    Boundary {
        id: prayer.id,
        name: prayer.name,
        kind: "prayer",
        minutes: prayer.minutes,
        day_offset,
    }
}

fn sleep_boundary(minutes: i64, day_offset: i64) -> Boundary {
    Boundary {
        id: "sleep",
        name: "Sleep",
        kind: "sleep",
        minutes,
        day_offset,
    }
}

fn absolute_seconds(boundary: &Boundary) -> i64 {
    boundary.day_offset * 86_400 + boundary.minutes * 60
}

/// Day offset of the sleep boundary that ends the evening of the display date.
///
/// A bedtime earlier than Fajr (for example `00:30`) belongs to the previous
/// evening, so it falls on the next calendar day.
pub fn evening_sleep_offset(sleep_minutes: i64, fajr_minutes: i64) -> i64 {
    i64::from(sleep_minutes < fajr_minutes)
}

pub fn timeline(
    today: &[PrayerTime],
    tomorrow_fajr: &PrayerTime,
    sleep_minutes: i64,
    now_seconds: i64,
) -> Result<Timeline, String> {
    let fajr_minutes = today
        .iter()
        .find(|prayer| prayer.id == "fajr")
        .ok_or("missing fajr")?
        .minutes;
    let sleep_offset = evening_sleep_offset(sleep_minutes, fajr_minutes);
    let mut candidates: Vec<Boundary> = today
        .iter()
        .filter(|prayer| prayer.id != "sunrise")
        .map(|prayer| prayer_boundary(prayer, 0))
        .collect();
    // Yesterday's Isha starts the block that a post-midnight bedtime ends.
    // Today's Isha is a close approximation of yesterday's.
    if let Some(isha) = today.iter().find(|prayer| prayer.id == "isha") {
        candidates.push(prayer_boundary(isha, -1));
    }
    candidates.push(sleep_boundary(sleep_minutes, sleep_offset - 1));
    candidates.push(sleep_boundary(sleep_minutes, sleep_offset));
    candidates.push(prayer_boundary(tomorrow_fajr, 1));

    let next = candidates
        .iter()
        .filter(|boundary| absolute_seconds(boundary) > now_seconds)
        .min_by_key(|boundary| absolute_seconds(boundary))
        .cloned()
        .ok_or("missing next boundary")?;
    let current_start = candidates
        .into_iter()
        .filter(|boundary| absolute_seconds(boundary) <= now_seconds)
        .max_by_key(absolute_seconds)
        .ok_or("missing current block start")?;
    let remaining_seconds = absolute_seconds(&next) - now_seconds;

    Ok(Timeline {
        next,
        current_start,
        remaining_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prayers() -> Vec<PrayerTime> {
        vec![
            PrayerTime {
                id: "fajr",
                name: "Fajr",
                minutes: 6 * 60,
            },
            PrayerTime {
                id: "sunrise",
                name: "Sunrise",
                minutes: 7 * 60 + 30,
            },
            PrayerTime {
                id: "dhuhr",
                name: "Dhuhr",
                minutes: 12 * 60,
            },
            PrayerTime {
                id: "asr",
                name: "Asr",
                minutes: 15 * 60,
            },
            PrayerTime {
                id: "maghrib",
                name: "Maghrib",
                minutes: 18 * 60,
            },
            PrayerTime {
                id: "isha",
                name: "Isha",
                minutes: 19 * 60 + 30,
            },
        ]
    }

    #[test]
    fn sleep_ends_the_post_isha_block() {
        let tomorrow_fajr = PrayerTime {
            id: "fajr",
            name: "Fajr",
            minutes: 6 * 60,
        };
        let result = timeline(&prayers(), &tomorrow_fajr, 23 * 60, 21 * 3600).unwrap();
        assert_eq!(result.current_start.id, "isha");
        assert_eq!(result.next.id, "sleep");
        assert_eq!(result.remaining_seconds, 2 * 3600);
    }

    #[test]
    fn after_sleep_is_the_sleep_to_fajr_block() {
        let tomorrow_fajr = PrayerTime {
            id: "fajr",
            name: "Fajr",
            minutes: 6 * 60,
        };
        let result = timeline(&prayers(), &tomorrow_fajr, 23 * 60, 23 * 3600 + 30 * 60).unwrap();
        assert_eq!(result.current_start.id, "sleep");
        assert_eq!(result.next.id, "fajr");
        assert_eq!(result.next.day_offset, 1);
    }

    #[test]
    fn sleep_block_begins_exactly_at_the_configured_time() {
        let tomorrow_fajr = PrayerTime {
            id: "fajr",
            name: "Fajr",
            minutes: 6 * 60,
        };
        let result = timeline(&prayers(), &tomorrow_fajr, 23 * 60, 23 * 3600).unwrap();
        assert_eq!(result.current_start.id, "sleep");
        assert_eq!(result.next.id, "fajr");
        assert_eq!(result.remaining_seconds, 7 * 3600);
    }

    #[test]
    fn before_fajr_uses_yesterdays_sleep_as_the_block_start() {
        let tomorrow_fajr = PrayerTime {
            id: "fajr",
            name: "Fajr",
            minutes: 6 * 60,
        };
        let result = timeline(&prayers(), &tomorrow_fajr, 23 * 60, 2 * 3600).unwrap();
        assert_eq!(result.current_start.id, "sleep");
        assert_eq!(result.current_start.day_offset, -1);
        assert_eq!(result.next.id, "fajr");
        assert_eq!(result.next.day_offset, 0);
    }

    fn fajr() -> PrayerTime {
        PrayerTime {
            id: "fajr",
            name: "Fajr",
            minutes: 6 * 60,
        }
    }

    #[test]
    fn after_midnight_bedtime_is_next_after_isha() {
        let result = timeline(&prayers(), &fajr(), 30, 21 * 3600).unwrap();
        assert_eq!(result.current_start.id, "isha");
        assert_eq!(result.next.id, "sleep");
        assert_eq!(result.next.day_offset, 1);
        assert_eq!(result.remaining_seconds, 3 * 3600 + 30 * 60);
    }

    #[test]
    fn after_midnight_but_before_bedtime_is_still_awake() {
        let result = timeline(&prayers(), &fajr(), 30, 10 * 60).unwrap();
        assert_eq!(result.current_start.id, "isha");
        assert_eq!(result.current_start.day_offset, -1);
        assert_eq!(result.next.id, "sleep");
        assert_eq!(result.remaining_seconds, 20 * 60);
    }

    #[test]
    fn after_post_midnight_bedtime_is_sleeping_until_fajr() {
        let result = timeline(&prayers(), &fajr(), 30, 3600).unwrap();
        assert_eq!(result.current_start.id, "sleep");
        assert_eq!(result.next.id, "fajr");
        assert_eq!(result.next.day_offset, 0);
    }

    #[test]
    fn clock_parser_is_strict() {
        assert_eq!(parse_clock_minutes("23:05"), Ok(23 * 60 + 5));
        for invalid in ["7:00", "24:00", "23:60", "ab:cd", " 23:00"] {
            assert!(parse_clock_minutes(invalid).is_err(), "{invalid}");
        }
    }
}
