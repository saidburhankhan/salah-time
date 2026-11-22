//! Typed request parameters and result DTOs for protocol version 1.

use chrono::NaiveDate;
use salah_core::config::Config;
use salah_core::export::{Export, ImportSummary};
use salah_core::planner::{
    AxisTick, BlockPlan, DayPlan, DayReport, DayTotals, ItemChanges, NextBoundary, Now, Preview,
    RunningTimer, Stats, Today, Totals,
};
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

/// A day row plus everything a UI would otherwise derive: entity version,
/// display strings, the next date the item applies, and a timer snapshot.
#[derive(Clone, Debug, Serialize)]
pub struct ItemRow {
    #[serde(flatten)]
    pub row: DayItem,
    pub version: i64,
    pub schedule_text: String,
    pub next_occurrence: Option<NaiveDate>,
    pub detail_text: String,
    pub timer: ItemTimerRow,
    pub carried: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ItemTimerRow {
    pub running: bool,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub minutes_including_running: i64,
}

impl From<&DayItem> for ItemRow {
    fn from(row: &DayItem) -> Self {
        Self {
            row: row.clone(),
            version: row.version,
            schedule_text: row.schedule_text.clone(),
            next_occurrence: row.next_occurrence,
            detail_text: row.detail_text.clone(),
            timer: ItemTimerRow {
                running: row.timer.running,
                started_at: row.timer.started_at,
                minutes_including_running: row.timer.minutes_including_running,
            },
            carried: row.carried,
        }
    }
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
// read views: the CLI keeps its frozen shapes, RPC adds display projections
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct NextBoundaryRow {
    pub id: BoundaryId,
    pub name: &'static str,
    pub time: String,
    pub at: chrono::DateTime<chrono::Utc>,
    pub remaining_seconds: i64,
}

impl From<NextBoundary> for NextBoundaryRow {
    fn from(next: NextBoundary) -> Self {
        Self {
            id: next.id,
            name: next.name,
            time: next.time,
            at: next.at,
            remaining_seconds: next.remaining_seconds,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct NowRow {
    pub block: Option<BoundaryId>,
    pub sleeping: bool,
    pub next: NextBoundaryRow,
}

impl From<Now> for NowRow {
    fn from(now: Now) -> Self {
        Self {
            block: now.block,
            sleeping: now.sleeping,
            next: now.next.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockPlanRow {
    pub start: BoundaryId,
    pub end: BoundaryId,
    pub items: Vec<ItemRow>,
}

impl From<BlockPlan> for BlockPlanRow {
    fn from(block: BlockPlan) -> Self {
        Self {
            start: block.start,
            end: block.end,
            items: block.items.iter().map(ItemRow::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DayPlanView {
    pub day: NaiveDate,
    pub is_today: bool,
    pub now: Option<NowRow>,
    pub prayers: Vec<salah_core::planner::PrayerRow>,
    pub sunrise: Option<String>,
    pub night_start: String,
    pub sleep: salah_core::planner::SleepRow,
    pub blocks: Vec<BlockPlanRow>,
    pub open: usize,
}

impl From<DayPlan> for DayPlanView {
    fn from(plan: DayPlan) -> Self {
        Self {
            day: plan.day,
            is_today: plan.is_today,
            now: plan.now.map(NowRow::from),
            prayers: plan.prayers,
            sunrise: plan.sunrise,
            night_start: plan.night_start,
            sleep: plan.sleep,
            blocks: plan.blocks.into_iter().map(BlockPlanRow::from).collect(),
            open: plan.open,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TodayView {
    pub day: NaiveDate,
    pub block: Option<BoundaryId>,
    pub planning_block: salah_domain::day::BlockKey,
    pub current: Vec<ItemRow>,
    pub open: usize,
    pub items: Vec<ItemRow>,
    pub upcoming: Vec<ItemRow>,
    pub prayer_marks: Vec<BoundaryId>,
    pub timer: Option<RunningTimer>,
}

impl From<Today> for TodayView {
    fn from(today: Today) -> Self {
        Self {
            day: today.day,
            block: today.block,
            planning_block: today.planning_block,
            current: today.current.iter().map(ItemRow::from).collect(),
            open: today.open,
            items: today.items.iter().map(ItemRow::from).collect(),
            upcoming: today.upcoming.iter().map(ItemRow::from).collect(),
            prayer_marks: today.prayer_marks,
            timer: today.timer,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct NextPrayerView {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
    pub time: String,
    pub date: String,
    pub remaining: String,
    pub remaining_short: String,
    pub at: chrono::DateTime<chrono::Utc>,
    pub remaining_seconds: i64,
}

#[derive(Debug, Serialize)]
pub struct StatusView {
    pub schema_version: u32,
    pub location: String,
    pub method: String,
    pub method_name: &'static str,
    pub source: &'static str,
    pub source_name: &'static str,
    pub source_updated: Option<String>,
    pub date: String,
    pub day: String,
    pub sleep_time: String,
    pub label: String,
    pub next: NextPrayerView,
    pub current_block: salah_core::status::CurrentBlock,
    pub prayers: Vec<salah_core::status::DisplayPrayer>,
    pub boundaries: Vec<salah_core::status::DisplayBoundary>,
    pub planner: Option<TodayView>,
    pub planner_error: Option<String>,
}

impl From<Status> for StatusView {
    fn from(status: Status) -> Self {
        let next = status.next;
        Self {
            schema_version: status.schema_version,
            location: status.location,
            method: status.method,
            method_name: status.method_name,
            source: status.source,
            source_name: status.source_name,
            source_updated: status.source_updated,
            date: status.date,
            day: status.day,
            sleep_time: status.sleep_time,
            label: status.label,
            next: NextPrayerView {
                id: next.id,
                name: next.name,
                kind: next.kind,
                time: next.time,
                date: next.date,
                remaining: next.remaining,
                remaining_short: next.remaining_short,
                at: next.at,
                remaining_seconds: next.remaining_seconds,
            },
            current_block: status.current_block,
            prayers: status.prayers,
            boundaries: status.boundaries,
            planner: status.planner.map(TodayView::from),
            planner_error: status.planner_error,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DayTotalsRow {
    #[serde(flatten)]
    pub totals: DayTotals,
    pub tooltip: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ItemStatsRow {
    #[serde(flatten)]
    pub stats: salah_core::planner::ItemStats,
    pub schedule_text: String,
    pub next_occurrence: Option<NaiveDate>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StatsView {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub days: usize,
    pub items: Vec<ItemStatsRow>,
    pub prayers: Vec<salah_core::planner::PrayerStats>,
    pub marked_total: usize,
    pub prayer_slots: usize,
    pub daily: Vec<DayTotalsRow>,
    pub totals: Totals,
    pub time_ticks: Vec<AxisTick>,
}

impl From<Stats> for StatsView {
    fn from(stats: Stats) -> Self {
        Self {
            from: stats.from,
            to: stats.to,
            days: stats.days,
            items: stats
                .items
                .into_iter()
                .map(|stats| ItemStatsRow {
                    schedule_text: stats.schedule_text.clone(),
                    next_occurrence: stats.next_occurrence,
                    stats,
                })
                .collect(),
            prayers: stats.prayers,
            marked_total: stats.marked_total,
            prayer_slots: stats.prayer_slots,
            daily: stats
                .daily
                .into_iter()
                .map(|totals| DayTotalsRow {
                    tooltip: totals.tooltip.clone(),
                    totals,
                })
                .collect(),
            totals: stats.totals,
            time_ticks: stats.time_ticks,
        }
    }
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

#[derive(Clone, Debug, Deserialize)]
pub struct PreviewParams {
    pub block: BoundaryId,
    pub repeat: Repeat,
    #[serde(default)]
    pub weekdays: Option<Weekdays>,
    pub measure: Measure,
    #[serde(default)]
    pub target: Option<i64>,
    pub date: NaiveDate,
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

pub type StatusResult = StatusView;
pub type DayResult = DayPlanView;
pub type ItemsResult = Vec<ItemRecord>;
pub type StatsResult = StatsView;
pub type HistoryResult = Vec<DayReport>;
pub type ParseResult = Draft;
pub type PreviewResult = Preview;
pub type ConfigResult = Config;
pub type ExportResult = Export;
pub type TodayResult = TodayView;
