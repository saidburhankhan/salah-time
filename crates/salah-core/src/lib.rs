//! Application layer: validated configuration, location sync policy, the
//! status projection, and the tracking use cases. Every client (CLI, socket
//! daemon, widget, desktop app) reaches these through `salah-protocol`.
//!
//! Mutations take an open [`WriteTx`], so the caller decides the transaction
//! and its idempotency record; reads take a [`Store`].

pub mod config;
pub mod day;
pub mod items;
pub mod location;
pub mod marks;
pub mod metrics;
pub mod projects;
pub mod quick;
pub mod sessions;
pub mod stats;
pub mod status;
pub mod tags;

mod guard;

pub use guard::stamp;
pub use salah_store::{Store, WriteTx};

#[cfg(test)]
mod test_support {
    use crate::Store;
    use crate::config::Config;
    use crate::projects::{self, NewProject};
    use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone, Utc};
    use salah_domain::{runtime::UuidV4, tracking::Project};

    /// No official cache exists for this method, so tests never read user data.
    pub fn config() -> Config {
        Config {
            method: "mwl".into(),
            ..Config::default()
        }
    }

    /// Tashkent local time (UTC+5) in November 2026.
    pub fn at(day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        FixedOffset::east_opt(5 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 11, day, hour, minute, 0)
            .unwrap()
            .with_timezone(&Utc)
    }

    pub fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
    }

    pub fn ids() -> &'static UuidV4 {
        &UuidV4
    }

    /// A project asking for the built-in metrics, both required.
    pub fn project(store: &mut Store, name: &str) -> Project {
        store
            .write(|tx| {
                projects::add(
                    tx,
                    at(20, 9, 0),
                    ids(),
                    NewProject {
                        name: name.into(),
                        ..NewProject::default()
                    },
                )
            })
            .unwrap()
    }
}
