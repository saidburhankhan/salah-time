//! Typed request parameters and result DTOs for protocol version 1.

use chrono::NaiveDate;
use salah_core::config::Config;
use salah_core::export::{Export, ImportSummary};
use salah_core::planner::{DayPlan, DayReport, ItemChanges, Stats, Today};
use salah_core::quick::Draft;
use salah_core::status::Status;
use salah_domain::day::BoundaryId;
use salah_domain::item::{DayItem, Entry, Item, Measure, PrayerMark, Repeat, Weekdays};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// hello
// ---------------------------------------------------------------------------

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

/// `subscribe` asks the transport to push `changed` notifications on this
/// connection. The stdio transport accepts the request but cannot push, so it
/// only returns the current revision.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SubscribeParams {
    /// Supported topic: `changes`. An empty list means `changes`.
    #[serde(default)]
    pub topics: Vec<String>,
    /// The last revision the client already saw; returned in the result so a
    /// reconnecting client can refetch when it has a gap.
    #[serde(default)]
    pub after_revision: Option<i64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SubscribeResult {
    pub revision: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_revision: Option<i64>,
}

// ---------------------------------------------------------------------------
// read results
// ---------------------------------------------------------------------------

/// A raw stored item plus its concurrency version. The version is not part of
/// the frozen CLI JSON, so it is added explicitly on the wire.
#[derive(Clone, Debug, Serialize)]
pub struct ItemRecord {
    #[serde(flatten)]
    pub item: Item,
    pub version: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntryRecord {
    #[serde(flatten)]
    pub entry: Entry,
    pub version: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarkRecord {
    #[serde(flatten)]
    pub mark: PrayerMark,
    pub version: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ItemRow {
    #[serde(flatten)]
    pub row: DayItem,
    pub version: i64,
}

/// Everything a mutation changed, as the projection the UI renders.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Changed {
    pub items: Vec<ItemRow>,
    pub entries: Vec<EntryRecord>,
    pub marks: Vec<MarkRecord>,
    pub deleted: Vec<String>,
}

impl Changed {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
            && self.entries.is_empty()
            && self.marks.is_empty()
            && self.deleted.is_empty()
    }
}

/// Canonical mutation response. `revision` is the store-wide counter, not a
/// per-entity precondition.
#[derive(Clone, Debug, Serialize)]
pub struct MutationResult {
    pub revision: i64,
    pub op_id: String,
    pub changed: Changed,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undone: Option<usize>,
}

// ---------------------------------------------------------------------------
// read params
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Deserialize)]
pub struct EmptyParams {}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DayParams {
    #[serde(default)]
    pub date: Option<NaiveDate>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct StatsParams {
    #[serde(default)]
    pub from: Option<NaiveDate>,
    #[serde(default)]
    pub to: Option<NaiveDate>,
    #[serde(default)]
    pub days: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct HistoryParams {
    #[serde(default)]
    pub from: Option<NaiveDate>,
    #[serde(default)]
    pub to: Option<NaiveDate>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ParseParams {
    pub text: String,
}

// ---------------------------------------------------------------------------
// mutation params
// ---------------------------------------------------------------------------

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
    pub measure: Option<Measure>,
    #[serde(default)]
    pub target: Option<i64>,
    #[serde(default)]
    pub date: Option<NaiveDate>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ItemEditParams {
    pub op_id: String,
    pub id: String,
    #[serde(default)]
    pub expected_version: Option<i64>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub block: Option<BoundaryId>,
    /// `null` clears the target; an absent field leaves it unchanged.
    #[serde(default)]
    pub target: Option<Option<i64>>,
    #[serde(default)]
    pub repeat: Option<Repeat>,
    #[serde(default)]
    pub weekdays: Option<Weekdays>,
    #[serde(default)]
    pub measure: Option<Measure>,
}

impl ItemEditParams {
    pub fn changes(&self) -> ItemChanges {
        ItemChanges {
            title: self.title.clone(),
            block: self.block,
            target: self.target,
            repeat: self.repeat,
            weekdays: self.weekdays,
            measure: self.measure,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ItemMoveParams {
    pub op_id: String,
    pub id: String,
    pub block: BoundaryId,
    /// Compatibility with the CLI's numeric index. Prefer `after_id` or
    /// `before_id`; at most one positioning field may be set.
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
pub struct ItemRemoveParams {
    pub op_id: String,
    pub id: String,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntryDoneParams {
    pub op_id: String,
    pub item_id: String,
    #[serde(default)]
    pub date: Option<NaiveDate>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntryUndoParams {
    pub op_id: String,
    pub item_id: String,
    #[serde(default)]
    pub date: Option<NaiveDate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogKind {
    Count,
    Minutes,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntryLogParams {
    pub op_id: String,
    pub item_id: String,
    #[serde(default)]
    pub kind: Option<LogKind>,
    pub value: i64,
    #[serde(default)]
    pub date: Option<NaiveDate>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TimerStartParams {
    pub op_id: String,
    pub item_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TimerStopParams {
    pub op_id: String,
    #[serde(default)]
    pub item_id: Option<String>,
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

    /// Applies the changes with the same manual-location rule as the CLI.
    pub fn apply(&self, mut config: Config) -> Config {
        let manual_location = self.location.is_some()
            || self.latitude.is_some()
            || self.longitude.is_some()
            || self.timezone_offset.is_some();
        let explicit_auto_location = self.auto_location.is_some();
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
        if manual_location && !explicit_auto_location {
            config.auto_location = false;
            config.last_location_sync = None;
        }
        config
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ImportParams {
    pub data: Export,
    #[serde(default)]
    pub op_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportResult {
    pub revision: i64,
    pub summary: ImportSummary,
}

// ---------------------------------------------------------------------------
// Convenience result aliases keep the dispatcher signatures readable.
// ---------------------------------------------------------------------------

pub type StatusResult = Status;
pub type DayResult = DayPlan;
pub type ItemsResult = Vec<ItemRecord>;
pub type StatsResult = Stats;
pub type HistoryResult = Vec<DayReport>;
pub type ParseResult = Draft;
pub type ConfigResult = Config;
pub type ExportResult = Export;
pub type TodayResult = Today;
