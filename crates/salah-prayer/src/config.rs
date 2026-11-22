use salah_domain::error::Error;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Config {
    pub location: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone_offset: f64,
    pub method: String,
    pub asr: String,
    #[serde(default = "default_sleep_time")]
    pub sleep_time: String,
    #[serde(default = "default_auto_location")]
    pub auto_location: bool,
    #[serde(default)]
    pub last_location_sync: Option<String>,
    #[serde(default)]
    pub last_location_provider: Option<String>,
}

const fn default_auto_location() -> bool {
    true
}

fn default_sleep_time() -> String {
    "23:00".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            location: "Tashkent".into(),
            latitude: 41.2995,
            longitude: 69.2401,
            timezone_offset: 5.0,
            method: "uzbekistan".into(),
            asr: "hanafi".into(),
            sleep_time: default_sleep_time(),
            auto_location: true,
            last_location_sync: None,
            last_location_provider: None,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), Error> {
        if self.location.trim().is_empty() {
            return Err("location cannot be empty".into());
        }
        if !(-90.0..=90.0).contains(&self.latitude) {
            return Err("latitude must be between -90 and 90".into());
        }
        if !(-180.0..=180.0).contains(&self.longitude) {
            return Err("longitude must be between -180 and 180".into());
        }
        if !(-14.0..=14.0).contains(&self.timezone_offset) {
            return Err("timezone offset must be between -14 and 14".into());
        }
        if crate::calculation::method(&self.method).is_none() {
            return Err(format!("unknown method: {}", self.method).into());
        }
        if self.asr != "standard" && self.asr != "hanafi" {
            return Err("asr must be standard or hanafi".into());
        }
        salah_domain::day::parse_clock_minutes(&self.sleep_time)
            .map_err(|_| "sleep time must use 24-hour HH:MM format".to_string())?;
        if let Some(timestamp) = &self.last_location_sync
            && (timestamp.len() > 64
                || timestamp.chars().any(char::is_control)
                || chrono::DateTime::parse_from_rfc3339(timestamp).is_err())
        {
            return Err("invalid last location sync timestamp".into());
        }
        if let Some(provider) = &self.last_location_provider
            && provider != "geo.omarchy.org"
            && provider != "ipapi.co"
            && provider != "omarchy-weather"
        {
            return Err("invalid location provider".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_finite_numbers_are_rejected() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let config = Config {
                latitude: value,
                ..Config::default()
            };
            assert!(config.validate().is_err());
            let config = Config {
                longitude: value,
                ..Config::default()
            };
            assert!(config.validate().is_err());
            let config = Config {
                timezone_offset: value,
                ..Config::default()
            };
            assert!(config.validate().is_err());
        }
    }
}
