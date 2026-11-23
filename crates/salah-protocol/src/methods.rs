//! Request parameters and result shapes of protocol version 1. Entity and
//! view types come from `salah-core`; this module only adds the envelopes.

use chrono::NaiveDate;
use salah_core::config::Config;
use salah_core::epics::{EpicChanges, NewEpic};
use salah_core::items::{ItemChanges, ItemView};
use salah_core::metrics::{MetricChanges, NewMetric};
use salah_core::projects::{NewProject, ProjectChanges, ProjectView};
use salah_core::sessions::{NewLog, SessionChanges, SessionQuery, SessionView, Stop, Target};
use salah_core::stats::measures::Measures;
use salah_core::stats::series::{Interval, Split};
use salah_core::stats::table::Group;
use salah_domain::day::BoundaryId;
use salah_domain::epic::{Epic, EpicStatus};
use salah_domain::item::{PrayerMark, Repeat, Weekdays};
use salah_domain::tracking::{Metric, Tag};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct HelloParams {
    /// Protocol major versions the client can speak.
    pub protocol: Vec<u32>,
    #[serde(default)]
    pub client: String,
    #[serde(default)]
    pub actor: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct HelloResult {
    pub protocol: u32,
    pub server: String,
    pub capabilities: Vec<String>,
    pub revision: i64,
}

/// The stdio transport accepts `subscribe` but cannot push, so it only
/// returns the current revision; the socket daemon registers the connection.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SubscribeParams {
    /// `changes` and/or `clock`; empty means `changes`.
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub after_revision: Option<i64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SubscribeResult {
    pub revision: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_revision: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DayParams {
    #[serde(default)]
    pub date: Option<NaiveDate>,
}

/// `from` and `to` together, or `days` ending today (default 30).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ScopeParams {
    #[serde(default)]
    pub from: Option<NaiveDate>,
    #[serde(default)]
    pub to: Option<NaiveDate>,
    #[serde(default)]
    pub days: Option<i64>,
    #[serde(default)]
    pub tag_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub epic_id: Option<String>,
    #[serde(default)]
    pub item_id: Option<String>,
}

/// `stats.get`: the range and filter plus the session bounds that narrow it.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct StatsParams {
    #[serde(flatten)]
    pub scope: ScopeParams,
    #[serde(flatten)]
    pub measures: Measures,
}

/// `stats.get`'s range and filter plus the row grouping (default `week`).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct StatsTableParams {
    #[serde(flatten)]
    pub stats: StatsParams,
    #[serde(default)]
    pub group: Group,
}

/// `stats.table`'s range and filter plus the period (default `day`), an
/// optional split and the measure ids, one series each per split key.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct StatsSeriesParams {
    #[serde(flatten)]
    pub stats: StatsParams,
    #[serde(default)]
    pub interval: Interval,
    #[serde(default)]
    pub split: Option<Split>,
    #[serde(default)]
    pub measures: Vec<String>,
}

/// `stats.get`'s range and filter plus the Notes page predicates and cursor.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SessionsListParams {
    #[serde(flatten)]
    pub stats: ScopeParams,
    #[serde(flatten)]
    pub query: SessionQuery,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ListParams {
    #[serde(default)]
    pub archived: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ParseParams {
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PreviewParams {
    pub block: BoundaryId,
    pub repeat: Repeat,
    #[serde(default)]
    pub weekdays: Option<Weekdays>,
    #[serde(default)]
    pub planned_minutes: Option<i64>,
    pub date: NaiveDate,
}

/// `title`, or `quick_text` parsed like the quick-add line (explicit fields
/// override what the text says).
#[derive(Clone, Debug, Deserialize)]
pub struct ItemAddParams {
    pub op_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub quick_text: Option<String>,
    #[serde(default)]
    pub block: Option<BoundaryId>,
    #[serde(default)]
    pub repeat: Option<Repeat>,
    #[serde(default)]
    pub weekdays: Option<Weekdays>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub epic_id: Option<String>,
    #[serde(default)]
    pub metric_ids: Option<Vec<String>>,
    #[serde(default)]
    pub planned_minutes: Option<i64>,
    #[serde(default)]
    pub date: Option<NaiveDate>,
    /// A one-time task without a day or block.
    #[serde(default)]
    pub backlog: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ItemEditParams {
    pub op_id: String,
    pub id: String,
    #[serde(default)]
    pub expected_version: Option<i64>,
    #[serde(flatten)]
    pub changes: ItemChanges,
}

/// At most one of `index`, `after_id`, `before_id`; none means the top.
#[derive(Clone, Debug, Deserialize)]
pub struct ItemMoveParams {
    pub op_id: String,
    pub id: String,
    /// Required; `null` moves within the backlog.
    #[serde(deserialize_with = "Option::deserialize")]
    pub block: Option<BoundaryId>,
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(default)]
    pub after_id: Option<String>,
    #[serde(default)]
    pub before_id: Option<String>,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ItemScheduleParams {
    pub op_id: String,
    pub id: String,
    pub day: NaiveDate,
    pub block: BoundaryId,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct BacklogListParams {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub tag_id: Option<String>,
}

/// `item.remove`, `item.backlog`, `tag.remove`, `session.remove`.
#[derive(Clone, Debug, Deserialize)]
pub struct RemoveParams {
    pub op_id: String,
    pub id: String,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PrayerParams {
    pub op_id: String,
    pub prayer: BoundaryId,
    #[serde(default)]
    pub date: Option<NaiveDate>,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EpicsListParams {
    pub project_id: Option<String>,
    pub status: Option<EpicStatus>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EpicAddParams {
    pub op_id: String,
    #[serde(flatten)]
    pub epic: NewEpic,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EpicEditParams {
    pub op_id: String,
    pub id: String,
    pub expected_version: Option<i64>,
    #[serde(flatten)]
    pub changes: EpicChanges,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProjectAddParams {
    pub op_id: String,
    #[serde(flatten)]
    pub project: NewProject,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProjectEditParams {
    pub op_id: String,
    pub id: String,
    #[serde(default)]
    pub expected_version: Option<i64>,
    #[serde(flatten)]
    pub changes: ProjectChanges,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TagAddParams {
    pub op_id: String,
    pub name: String,
    pub color: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TagEditParams {
    pub op_id: String,
    pub id: String,
    #[serde(default)]
    pub expected_version: Option<i64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MetricAddParams {
    pub op_id: String,
    #[serde(flatten)]
    pub metric: NewMetric,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MetricEditParams {
    pub op_id: String,
    pub id: String,
    #[serde(default)]
    pub expected_version: Option<i64>,
    #[serde(flatten)]
    pub changes: MetricChanges,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionStartParams {
    pub op_id: String,
    #[serde(flatten)]
    pub target: Target,
}

/// Stops `id`, or whichever session runs.
#[derive(Clone, Debug, Deserialize)]
pub struct SessionStopParams {
    pub op_id: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(flatten)]
    pub stop: Stop,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionBumpParams {
    pub op_id: String,
    #[serde(default)]
    pub id: Option<String>,
    pub metric_id: String,
    pub delta: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionNoteParams {
    pub op_id: String,
    #[serde(default)]
    pub id: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionLogParams {
    pub op_id: String,
    #[serde(flatten)]
    pub log: NewLog,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SessionEditParams {
    pub op_id: String,
    pub id: String,
    #[serde(default)]
    pub expected_version: Option<i64>,
    #[serde(flatten)]
    pub changes: SessionChanges,
}

/// Every entity a mutation wrote, as clients render it. `deleted` lists the
/// ids of removed items, tags, and sessions.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Changed {
    pub items: Vec<ItemView>,
    pub sessions: Vec<SessionView>,
    pub projects: Vec<ProjectView>,
    pub epics: Vec<Epic>,
    pub tags: Vec<Tag>,
    pub metrics: Vec<Metric>,
    pub marks: Vec<PrayerMark>,
    pub deleted: Vec<String>,
}

/// The result of every mutation. `revision` is the store-wide counter after
/// it, never a precondition.
#[derive(Clone, Debug, Serialize)]
pub struct MutationResult {
    pub revision: i64,
    pub op_id: String,
    pub changed: Changed,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConfigSetParams {
    #[serde(default)]
    pub op_id: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
    #[serde(default)]
    pub timezone_offset: Option<f64>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub asr: Option<String>,
    #[serde(default)]
    pub sleep_time: Option<String>,
    #[serde(default)]
    pub auto_location: Option<bool>,
}

impl ConfigSetParams {
    pub fn supplied(&self) -> bool {
        self.location.is_some()
            || self.latitude.is_some()
            || self.longitude.is_some()
            || self.timezone_offset.is_some()
            || self.method.is_some()
            || self.asr.is_some()
            || self.sleep_time.is_some()
            || self.auto_location.is_some()
    }

    /// Applies the changes. Setting the location by hand turns automatic
    /// location off unless `auto_location` is given too.
    pub fn apply(&self, mut config: Config) -> Config {
        let manual_location = self.location.is_some()
            || self.latitude.is_some()
            || self.longitude.is_some()
            || self.timezone_offset.is_some();
        if let Some(value) = &self.location {
            config.location = value.clone();
        }
        if let Some(value) = self.latitude {
            config.latitude = value;
        }
        if let Some(value) = self.longitude {
            config.longitude = value;
        }
        if let Some(value) = self.timezone_offset {
            config.timezone_offset = value;
        }
        if let Some(value) = &self.method {
            config.method = value.clone();
        }
        if let Some(value) = &self.asr {
            config.asr = value.clone();
        }
        if let Some(value) = &self.sleep_time {
            config.sleep_time = value.clone();
        }
        if let Some(value) = self.auto_location {
            config.auto_location = value;
        }
        if manual_location && self.auto_location.is_none() {
            config.auto_location = false;
            config.last_location_sync = None;
        }
        config
    }
}
