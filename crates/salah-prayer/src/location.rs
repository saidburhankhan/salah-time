use crate::config::Config;
use chrono::{DateTime, Duration, Utc};
use salah_domain::error::Error;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub const GEO_URL: &str = "https://geo.omarchy.org/";
pub const FALLBACK_GEO_URL: &str = "https://ipapi.co/json/";
const MAX_RESPONSE_BYTES: usize = 16 * 1024;
const REFRESH_HOURS: i64 = 6;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DetectedLocation {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Clone, Debug)]
pub struct FetchedLocation {
    pub location: DetectedLocation,
    pub timezone_offset: Option<f64>,
    pub provider: &'static str,
}

#[derive(Deserialize)]
struct IpApiResponse {
    city: String,
    latitude: f64,
    longitude: f64,
    utc_offset: Option<String>,
}

pub fn parse_response(bytes: &[u8]) -> Result<DetectedLocation, Error> {
    let text = validated_response_text(bytes)?;
    let mut detected: DetectedLocation = serde_json::from_str(text)
        .map_err(|e| format!("invalid location service response: {e}"))?;
    detected.name = detected.name.trim().to_string();
    validate_detected(&detected)?;
    Ok(detected)
}

fn validated_response_text(bytes: &[u8]) -> Result<&str, Error> {
    if bytes.is_empty() {
        return Err("location service returned an empty response".into());
    }
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("location service response is too large".into());
    }
    Ok(std::str::from_utf8(bytes)
        .map_err(|_| "location service returned invalid UTF-8".to_string())?)
}

pub fn parse_ipapi_response(bytes: &[u8]) -> Result<FetchedLocation, Error> {
    let response: IpApiResponse = serde_json::from_str(validated_response_text(bytes)?)
        .map_err(|e| format!("invalid fallback location response: {e}"))?;
    let location = DetectedLocation {
        name: response.city.trim().to_string(),
        latitude: response.latitude,
        longitude: response.longitude,
    };
    validate_detected(&location)?;
    let timezone_offset = response
        .utc_offset
        .as_deref()
        .map(parse_utc_offset)
        .transpose()?;
    Ok(FetchedLocation {
        location,
        timezone_offset,
        provider: "ipapi.co",
    })
}

pub fn parse_utc_offset(value: &str) -> Result<f64, Error> {
    let compact = value.trim().replace(':', "");
    let bytes = compact.as_bytes();
    if bytes.len() != 5
        || !matches!(bytes[0], b'+' | b'-')
        || !bytes[1..].iter().all(u8::is_ascii_digit)
    {
        return Err("invalid UTC offset from location service".into());
    }
    let hours: u8 = compact[1..3]
        .parse()
        .map_err(|_| "invalid UTC offset from location service")?;
    let minutes: u8 = compact[3..5]
        .parse()
        .map_err(|_| "invalid UTC offset from location service")?;
    if hours > 14 || minutes >= 60 || !minutes.is_multiple_of(15) || (hours == 14 && minutes != 0) {
        return Err("invalid UTC offset from location service".into());
    }
    let magnitude = f64::from(hours) + f64::from(minutes) / 60.0;
    Ok(if bytes[0] == b'-' {
        -magnitude
    } else {
        magnitude
    })
}

fn validate_detected(location: &DetectedLocation) -> Result<(), Error> {
    if location.name.is_empty() {
        return Err("detected location name is empty".into());
    }
    if location.name.len() > 120 || location.name.chars().any(char::is_control) {
        return Err("detected location name is invalid".into());
    }
    if !location.latitude.is_finite() || !(-90.0..=90.0).contains(&location.latitude) {
        return Err("detected latitude is invalid".into());
    }
    if !location.longitude.is_finite() || !(-180.0..=180.0).contains(&location.longitude) {
        return Err("detected longitude is invalid".into());
    }
    Ok(())
}

pub fn apply_detected(
    current: &Config,
    detected: DetectedLocation,
    timezone_offset: f64,
    synced_at: String,
    provider: &str,
) -> Result<Config, Error> {
    validate_detected(&detected)?;
    let mut updated = current.clone();
    updated.location = detected.name;
    updated.latitude = detected.latitude;
    updated.longitude = detected.longitude;
    updated.timezone_offset = timezone_offset;
    updated.auto_location = true;
    updated.last_location_sync = Some(synced_at);
    updated.last_location_provider = Some(provider.to_string());
    updated.validate()?;
    Ok(updated)
}

fn fetch_url(url: &str) -> Result<Vec<u8>, Error> {
    let max_response = MAX_RESPONSE_BYTES.to_string();
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
            "3",
            "--max-time",
            "5",
            "--max-filesize",
            &max_response,
            url,
        ])
        .output()
        .map_err(|e| Error::SourceUnavailable(format!("start curl: {e}")))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim().chars().take(300).collect::<String>();
        return Err(Error::SourceUnavailable(if detail.is_empty() {
            format!("location request failed with {}", output.status)
        } else {
            format!("location request failed: {detail}")
        }));
    }
    Ok(output.stdout)
}

pub fn fetch() -> Result<FetchedLocation, Error> {
    let primary = fetch_url(GEO_URL).and_then(|bytes| {
        parse_response(&bytes).map(|location| FetchedLocation {
            location,
            timezone_offset: None,
            provider: "geo.omarchy.org",
        })
    });
    match primary {
        Ok(found) => Ok(found),
        Err(primary_error) => Ok(fetch_url(FALLBACK_GEO_URL)
            .and_then(|bytes| parse_ipapi_response(&bytes))
            .map_err(|fallback_error| {
                format!(
                    "both location providers failed (geo.omarchy.org: {primary_error}; ipapi.co: {fallback_error})"
                )
            })?),
    }
}

fn omarchy_weather_location() -> Option<FetchedLocation> {
    let home = std::env::var_os("HOME")?;
    let path = PathBuf::from(home).join(".local/state/omarchy/settings/weather.json");
    let bytes = fs::read(path).ok()?;
    let location = parse_response(&bytes).ok()?;
    Some(FetchedLocation {
        location,
        timezone_offset: None,
        provider: "omarchy-weather",
    })
}

pub fn sync(
    current: &Config,
    now: DateTime<Utc>,
    local_timezone_offset: f64,
) -> Result<(Config, &'static str), Error> {
    // A location explicitly selected in Omarchy Weather is more precise and
    // more private than IP geolocation. Fall back to the online providers
    // only when Weather has no valid saved coordinates.
    let fetched = omarchy_weather_location().map_or_else(fetch, Ok)?;
    let provider = fetched.provider;
    let config = apply_detected(
        current,
        fetched.location,
        fetched.timezone_offset.unwrap_or(local_timezone_offset),
        now.to_rfc3339(),
        provider,
    )?;
    Ok((config, provider))
}

pub fn is_fresh(config: &Config, now: DateTime<Utc>) -> bool {
    config
        .last_location_sync
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .is_some_and(|then| {
            let age = now.signed_duration_since(then.with_timezone(&Utc));
            age >= Duration::zero() && age < Duration::hours(REFRESH_HOURS)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_trims_valid_response() {
        let found =
            parse_response(br#"{"name":"  Chilonzor  ","latitude":41.2756,"longitude":69.2034}"#)
                .unwrap();
        assert_eq!(found.name, "Chilonzor");
        assert_eq!(found.latitude, 41.2756);
    }

    #[test]
    fn rejects_untrusted_or_out_of_range_responses() {
        for raw in [
            br#"{}"#.as_slice(),
            br#"{"name":"X","latitude":91,"longitude":1}"#,
            br#"{"name":"X\nY","latitude":1,"longitude":1}"#,
            b"not json".as_slice(),
        ] {
            assert!(parse_response(raw).is_err(), "accepted {raw:?}");
        }
        assert!(parse_response(&vec![b'x'; MAX_RESPONSE_BYTES + 1]).is_err());
    }

    #[test]
    fn applying_location_preserves_calculation_preferences() {
        let config = Config {
            method: "mwl".into(),
            asr: "standard".into(),
            ..Config::default()
        };
        let updated = apply_detected(
            &config,
            DetectedLocation {
                name: "Samarkand".into(),
                latitude: 39.6542,
                longitude: 66.9597,
            },
            5.0,
            "2026-11-20T10:00:00Z".into(),
            "geo.omarchy.org",
        )
        .unwrap();
        assert_eq!(updated.method, "mwl");
        assert_eq!(updated.asr, "standard");
        assert_eq!(updated.location, "Samarkand");
        assert!(updated.auto_location);
    }

    #[test]
    fn parses_fallback_schema_and_remote_offset() {
        let found = parse_ipapi_response(
            br#"{"city":"Tashkent","latitude":41.31,"longitude":69.28,"utc_offset":"+0500","ip":"not-retained"}"#,
        )
        .unwrap();
        assert_eq!(found.location.name, "Tashkent");
        assert_eq!(found.timezone_offset, Some(5.0));
        assert_eq!(found.provider, "ipapi.co");
    }

    #[test]
    fn parses_quarter_hour_offsets_and_rejects_invalid_ones() {
        assert_eq!(parse_utc_offset("+05:45").unwrap(), 5.75);
        assert_eq!(parse_utc_offset("-0330").unwrap(), -3.5);
        assert_eq!(parse_utc_offset("+0000").unwrap(), 0.0);
        for value in ["0500", "+1460", "+1415", "+0561", "+0520", "hello"] {
            assert!(parse_utc_offset(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn freshness_requires_a_recent_valid_timestamp() {
        let now = DateTime::parse_from_rfc3339("2026-11-20T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut config = Config::default();
        assert!(!is_fresh(&config, now));
        config.last_location_sync = Some("2026-11-20T07:00:01Z".into());
        assert!(is_fresh(&config, now));
        config.last_location_sync = Some("2026-11-20T05:59:59Z".into());
        assert!(!is_fresh(&config, now));
        config.last_location_sync = Some("bad".into());
        assert!(!is_fresh(&config, now));
        config.last_location_sync = Some("2026-11-20T12:01:00Z".into());
        assert!(!is_fresh(&config, now));
    }
}
