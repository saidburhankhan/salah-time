use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EpicStatus {
    Active,
    Finished,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Epic {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub note: Option<String>,
    pub status: EpicStatus,
    pub finished_at: Option<DateTime<Utc>>,
    pub position: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub version: i64,
}

/// An epic as a session row names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EpicRef {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EpicView {
    #[serde(flatten)]
    pub epic: Epic,
    pub total_minutes: i64,
    pub sessions: i64,
    pub days_worked: i64,
    pub last_worked: Option<NaiveDate>,
    pub open_tasks: i64,
    pub done_tasks: i64,
}
