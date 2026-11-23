use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Note,
    Link,
    Reminder,
}

/// A note, link or reminder on exactly one item or epic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Resource {
    pub id: String,
    pub item_id: Option<String>,
    pub epic_id: Option<String>,
    pub kind: ResourceKind,
    pub title: Option<String>,
    pub body: Option<String>,
    pub url: Option<String>,
    pub remind_at: Option<DateTime<Utc>>,
    pub done: bool,
    pub position: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub version: i64,
}

/// How many live resources of each kind belong to one item: every note and
/// link, and the reminders not yet done.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ResourceCounts {
    pub notes: i64,
    pub links: i64,
    pub reminders: i64,
}

/// An open reminder whose time has come, with the label of
/// what it belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DueReminder {
    pub id: String,
    pub item_id: Option<String>,
    pub item_title: Option<String>,
    pub epic_id: Option<String>,
    pub epic_name: Option<String>,
    pub title: Option<String>,
    pub remind_at: DateTime<Utc>,
    /// Its scheduled time is strictly before the request instant.
    pub overdue: bool,
}
