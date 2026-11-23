use serde::Serialize;

/// How many live resources of each kind belong to one item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ResourceCounts {
    pub notes: i64,
    pub links: i64,
    pub reminders: i64,
}
