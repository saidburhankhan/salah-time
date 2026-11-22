use chrono::{Datelike, NaiveDate};
use salah_domain::day::PrayerTime;
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Method {
    pub id: &'static str,
    pub name: &'static str,
    pub fajr_angle: f64,
    pub isha: IshaRule,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IshaRule {
    Angle(f64),
    MinutesAfterMaghrib(i64),
}

pub const METHODS: [Method; 4] = [
    Method {
        id: "uzbekistan",
        // These angles reproduce the board's published Tashkent timetable to
        // within a few minutes, but they are not a documented official formula.
        // Keep the label honest until the board publishes calculation details.
        name: "Uzbekistan estimate (provisional)",
        fajr_angle: 16.0,
        isha: IshaRule::Angle(15.0),
    },
    Method {
        id: "mwl",
        name: "Muslim World League",
        fajr_angle: 18.0,
        isha: IshaRule::Angle(17.0),
    },
    Method {
        id: "umm_al_qura",
        name: "Umm al-Qura",
        fajr_angle: 18.5,
        isha: IshaRule::MinutesAfterMaghrib(90),
    },
    Method {
        id: "diyanet",
        name: "Türkiye Diyanet",
        fajr_angle: 18.0,
        isha: IshaRule::Angle(17.0),
    },
];

pub fn method(id: &str) -> Option<Method> {
    METHODS.iter().copied().find(|item| item.id == id)
}

pub fn calculate(
    date: NaiveDate,
    latitude: f64,
    longitude: f64,
    timezone: f64,
    calculation_method: Method,
    hanafi: bool,
) -> Vec<PrayerTime> {
    let jd = julian_date(date) - longitude / (15.0 * 24.0);
    let noon = mid_day(jd, 0.5);
    let fajr = sun_altitude_time(jd, latitude, -calculation_method.fajr_angle, 0.2, false);
    let sunrise = sun_altitude_time(jd, latitude, -0.833, 0.25, false);
    let asr_factor = if hanafi { 2.0 } else { 1.0 };
    let declination = sun_position(jd + 13.0 / 24.0).0;
    let asr_altitude = (1.0 / (asr_factor + (latitude - declination).to_radians().abs().tan()))
        .atan()
        .to_degrees();
    let asr = sun_altitude_time(jd, latitude, asr_altitude, 13.0 / 24.0, true);
    let maghrib = sun_altitude_time(jd, latitude, -0.833, 0.75, true);
    let isha = match calculation_method.isha {
        IshaRule::Angle(angle) => sun_altitude_time(jd, latitude, -angle, 0.8, true),
        IshaRule::MinutesAfterMaghrib(minutes) => maghrib + minutes as f64 / 60.0,
    };
    let adjust = timezone - longitude / 15.0;
    [
        ("fajr", "Fajr", fajr + adjust),
        ("sunrise", "Sunrise", sunrise + adjust),
        ("dhuhr", "Dhuhr", noon + adjust),
        ("asr", "Asr", asr + adjust),
        ("maghrib", "Maghrib", maghrib + adjust),
        ("isha", "Isha", isha + adjust),
    ]
    .into_iter()
    .map(|(id, name, hours)| PrayerTime {
        id,
        name,
        minutes: ((hours * 60.0).round() as i64).rem_euclid(1440),
    })
    .collect()
}

fn julian_date(date: NaiveDate) -> f64 {
    let mut year = date.year();
    let mut month = date.month() as i32;
    if month <= 2 {
        year -= 1;
        month += 12;
    }
    let a = (year as f64 / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (year as f64 + 4716.0)).floor()
        + (30.6001 * (month as f64 + 1.0)).floor()
        + date.day() as f64
        + b
        - 1524.5
}

fn sun_position(jd: f64) -> (f64, f64) {
    let d = jd - 2_451_545.0;
    let g = fix_angle(357.529 + 0.98560028 * d);
    let q = fix_angle(280.459 + 0.98564736 * d);
    let l = fix_angle(q + 1.915 * sin_deg(g) + 0.020 * sin_deg(2.0 * g));
    let e = 23.439 - 0.00000036 * d;
    let declination = (sin_deg(e) * sin_deg(l)).asin().to_degrees();
    let right_ascension = fix_hour((cos_deg(e) * sin_deg(l)).atan2(cos_deg(l)).to_degrees() / 15.0);
    (declination, q / 15.0 - right_ascension)
}

fn mid_day(jd: f64, fraction: f64) -> f64 {
    fix_hour(12.0 - sun_position(jd + fraction).1)
}

fn sun_altitude_time(
    jd: f64,
    latitude: f64,
    altitude: f64,
    fraction: f64,
    after_noon: bool,
) -> f64 {
    let declination = sun_position(jd + fraction).0;
    let noon = mid_day(jd, fraction);
    let hour_angle = altitude_hour_angle(latitude, declination, altitude)
        // Above the polar circles the requested solar altitude may not occur
        // at all. Clamping the cosine to [-1, 1] disguises that condition and
        // produces a made-up midnight or noon. Use a conventional nearest-
        // latitude estimate instead so the schedule remains ordered and the
        // result is explicit, deterministic, and useful year-round.
        .or_else(|| altitude_hour_angle(latitude.clamp(-48.5, 48.5), declination, altitude))
        .expect("48.5-degree fallback always has a daily solar crossing");
    if after_noon {
        noon + hour_angle
    } else {
        noon - hour_angle
    }
}

fn altitude_hour_angle(latitude: f64, declination: f64, altitude: f64) -> Option<f64> {
    let denominator = cos_deg(declination) * cos_deg(latitude);
    if denominator.abs() < f64::EPSILON {
        return None;
    }
    let ratio = (sin_deg(altitude) - sin_deg(declination) * sin_deg(latitude)) / denominator;
    if !(-1.0..=1.0).contains(&ratio) {
        return None;
    }
    Some(ratio.acos() * 180.0 / PI / 15.0)
}

fn fix_angle(value: f64) -> f64 {
    value.rem_euclid(360.0)
}
fn fix_hour(value: f64) -> f64 {
    value.rem_euclid(24.0)
}
fn sin_deg(value: f64) -> f64 {
    value.to_radians().sin()
}
fn cos_deg(value: f64) -> f64 {
    value.to_radians().cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minutes(times: &[PrayerTime], id: &str) -> i64 {
        times.iter().find(|time| time.id == id).unwrap().minutes
    }

    fn assert_strictly_ordered(times: &[PrayerTime]) {
        assert!(
            times
                .windows(2)
                .all(|pair| pair[0].minutes < pair[1].minutes),
            "prayer times are not ordered: {times:?}"
        );
    }

    #[test]
    fn tashkent_times_are_ordered_and_plausible() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        let times = calculate(
            date,
            41.2995,
            69.2401,
            5.0,
            method("uzbekistan").unwrap(),
            true,
        );
        assert_strictly_ordered(&times);
        assert!((240..480).contains(&times[0].minutes));
        assert!((960..1200).contains(&times[4].minutes));
    }

    #[test]
    fn tashkent_matches_muslim_board_reference_within_six_minutes() {
        // Muslim Board of Uzbekistan timetable for Tashkent, 2026-11-20:
        // Fajr 05:54, Sunrise 07:12, Dhuhr 12:14, Asr 15:18,
        // Maghrib 17:03, Isha 18:19. This is a reference comparison, not a
        // claim that the provisional 16°/15° angles are the board's formula.
        let times = calculate(
            NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
            41.2995,
            69.2401,
            5.0,
            method("uzbekistan").unwrap(),
            true,
        );
        let official = [354, 432, 734, 918, 1023, 1099];
        for (actual, expected) in times.iter().zip(official) {
            assert!(
                (actual.minutes - expected).abs() <= 6,
                "{} differs from the official reference: actual={}, expected={}",
                actual.name,
                actual.minutes,
                expected
            );
        }
    }
    #[test]
    fn hanafi_asr_is_later() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let std = calculate(date, 41.2995, 69.2401, 5.0, method("mwl").unwrap(), false);
        let hanafi = calculate(date, 41.2995, 69.2401, 5.0, method("mwl").unwrap(), true);
        assert!(hanafi[3].minutes > std[3].minutes);
    }

    #[test]
    fn greenwich_equinox_matches_published_solar_expectations() {
        // Around the March equinox at Greenwich, sunrise and sunset are close
        // to 06:00 and 18:00 UTC. The broad tolerance covers refraction-model
        // differences while catching longitude, timezone, and sign mistakes.
        let times = calculate(
            NaiveDate::from_ymd_opt(2026, 3, 20).unwrap(),
            51.4769,
            0.0,
            0.0,
            method("mwl").unwrap(),
            false,
        );
        assert!(
            (350..=380).contains(&minutes(&times, "sunrise")),
            "{times:?}"
        );
        assert!((715..=745).contains(&minutes(&times, "dhuhr")), "{times:?}");
        assert!(
            (1070..=1105).contains(&minutes(&times, "maghrib")),
            "{times:?}"
        );
    }

    #[test]
    fn tashkent_schedule_stays_ordered_through_the_year() {
        for month in 1..=12 {
            let date = NaiveDate::from_ymd_opt(2026, month, 15).unwrap();
            for method_id in ["uzbekistan", "mwl", "umm_al_qura", "diyanet"] {
                for hanafi in [false, true] {
                    let times = calculate(
                        date,
                        41.2995,
                        69.2401,
                        5.0,
                        method(method_id).unwrap(),
                        hanafi,
                    );
                    assert_strictly_ordered(&times);
                    assert!(times.iter().all(|time| (0..1440).contains(&time.minutes)));
                }
            }
        }
    }

    #[test]
    fn high_latitude_fallback_does_not_collapse_solar_events() {
        for date in [
            NaiveDate::from_ymd_opt(2026, 6, 21).unwrap(),
            NaiveDate::from_ymd_opt(2026, 12, 21).unwrap(),
        ] {
            let times = calculate(date, 69.6492, 18.9553, 1.0, method("mwl").unwrap(), true);
            assert_strictly_ordered(&times);
            assert_ne!(minutes(&times, "sunrise"), minutes(&times, "maghrib"));
        }
    }

    #[test]
    fn asr_variants_remain_between_noon_and_sunset() {
        for (date, latitude, longitude, timezone) in [
            (
                NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
                41.2995,
                69.2401,
                5.0,
            ),
            (
                NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
                40.7128,
                -74.006,
                -4.0,
            ),
            (
                NaiveDate::from_ymd_opt(2026, 12, 15).unwrap(),
                -33.8688,
                151.2093,
                11.0,
            ),
        ] {
            let standard = calculate(
                date,
                latitude,
                longitude,
                timezone,
                method("mwl").unwrap(),
                false,
            );
            let hanafi = calculate(
                date,
                latitude,
                longitude,
                timezone,
                method("mwl").unwrap(),
                true,
            );
            let noon = minutes(&standard, "dhuhr");
            let sunset = minutes(&standard, "maghrib");
            let standard_asr = minutes(&standard, "asr");
            let hanafi_asr = minutes(&hanafi, "asr");
            assert!(noon < standard_asr && standard_asr < hanafi_asr && hanafi_asr < sunset);
        }
    }
}
