use crate::{Store, WriteTx, codec};
use chrono::{DateTime, Utc};
use rusqlite::{Row, params};
use salah_domain::{
    error::Error,
    resource::{DueReminder, Resource, ResourceCounts},
};
use std::collections::BTreeMap;

/// Which item or epic resources belong to.
#[derive(Clone, Copy, Debug)]
pub enum Parent<'a> {
    Item(&'a str),
    Epic(&'a str),
}

impl<'a> Parent<'a> {
    fn column(self) -> (&'static str, &'a str) {
        match self {
            Parent::Item(id) => ("item_id", id),
            Parent::Epic(id) => ("epic_id", id),
        }
    }
}

fn resource(row: &Row) -> rusqlite::Result<Resource> {
    Ok(Resource {
        id: row.get("id")?,
        item_id: row.get("item_id")?,
        epic_id: row.get("epic_id")?,
        kind: codec::get_name(row, "kind")?,
        title: row.get("title")?,
        body: row.get("body")?,
        url: row.get("url")?,
        remind_at: codec::get_optional_timestamp(row, "remind_at")?,
        done: row.get("done")?,
        position: row.get("position")?,
        created_at: codec::get_timestamp(row, "created_at")?,
        updated_at: codec::get_timestamp(row, "updated_at")?,
        deleted_at: codec::get_optional_timestamp(row, "deleted_at")?,
        version: row.get("version")?,
    })
}

impl Store {
    pub fn resource(&self, id: &str) -> Result<Option<Resource>, Error> {
        self.one("SELECT * FROM resources WHERE id = ?1", [id], resource)
    }

    /// The live resources of one item or epic in position order.
    pub fn resources_of(&self, parent: Parent<'_>) -> Result<Vec<Resource>, Error> {
        let (column, id) = parent.column();
        self.all(
            &format!(
                "SELECT * FROM resources WHERE {column} = ?1 AND deleted_at IS NULL
                 ORDER BY position, id"
            ),
            [id],
            resource,
        )
    }

    /// The position after the parent's live resources.
    pub fn next_resource_position(&self, parent: Parent<'_>) -> Result<i64, Error> {
        let (column, id) = parent.column();
        self.conn
            .query_row(
                &format!(
                    "SELECT COALESCE(MAX(position) + 1, 0) FROM resources
                     WHERE {column} = ?1 AND deleted_at IS NULL"
                ),
                [id],
                |row| row.get(0),
            )
            .map_err(codec::storage_error)
    }

    /// Open reminders of live items and epics due at or before `now`, oldest first.
    pub fn due_reminders(&self, now: DateTime<Utc>) -> Result<Vec<DueReminder>, Error> {
        self.all(
            "SELECT r.id, r.item_id, i.title AS item_title, r.epic_id, e.name AS epic_name,
                    r.title, r.remind_at
             FROM resources r
             LEFT JOIN items i ON i.id = r.item_id
             LEFT JOIN epics e ON e.id = r.epic_id
             WHERE r.kind = 'reminder' AND r.done = 0 AND r.deleted_at IS NULL
               AND r.remind_at <= ?1
               AND (r.item_id IS NULL OR i.deleted_at IS NULL)
               AND (r.epic_id IS NULL OR e.deleted_at IS NULL)
             ORDER BY r.remind_at, r.id",
            [codec::timestamp(now)],
            |row| {
                let remind_at = codec::get_timestamp(row, "remind_at")?;
                Ok(DueReminder {
                    id: row.get("id")?,
                    item_id: row.get("item_id")?,
                    item_title: row.get("item_title")?,
                    epic_id: row.get("epic_id")?,
                    epic_name: row.get("epic_name")?,
                    title: row.get("title")?,
                    overdue: remind_at < now,
                    remind_at,
                })
            },
        )
    }

    /// Live resource counts per item that has any: every note and link, and
    /// the reminders not yet done.
    pub fn item_resource_counts(&self) -> Result<BTreeMap<String, ResourceCounts>, Error> {
        let rows = self.all(
            "SELECT item_id, kind, COUNT(*) AS n FROM resources
             WHERE item_id IS NOT NULL AND deleted_at IS NULL
                 AND (kind != 'reminder' OR done = 0)
             GROUP BY item_id, kind",
            [],
            |row| {
                Ok((
                    row.get::<_, String>("item_id")?,
                    row.get::<_, String>("kind")?,
                    row.get::<_, i64>("n")?,
                ))
            },
        )?;
        let mut counts = BTreeMap::<String, ResourceCounts>::new();
        for (item_id, kind, n) in rows {
            let entry = counts.entry(item_id).or_default();
            match kind.as_str() {
                "note" => entry.notes = n,
                "link" => entry.links = n,
                _ => entry.reminders = n,
            }
        }
        Ok(counts)
    }
}

impl WriteTx<'_> {
    pub fn put_resource(&self, r: &Resource) -> Result<i64, Error> {
        let (version, op) = self.next_version("resources", &r.id, r.deleted_at)?;
        self.execute(
            "INSERT INTO resources
                 (id, item_id, epic_id, kind, title, body, url, remind_at, done, position,
                  created_at, updated_at, deleted_at, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT (id) DO UPDATE SET
                 title = excluded.title, body = excluded.body, url = excluded.url,
                 remind_at = excluded.remind_at, done = excluded.done,
                 position = excluded.position, updated_at = excluded.updated_at,
                 deleted_at = excluded.deleted_at, version = excluded.version",
            params![
                r.id,
                r.item_id,
                r.epic_id,
                codec::name(&r.kind),
                r.title,
                r.body,
                r.url,
                r.remind_at.map(codec::timestamp),
                r.done,
                r.position,
                codec::timestamp(r.created_at),
                codec::timestamp(r.updated_at),
                r.deleted_at.map(codec::timestamp),
                version,
            ],
        )?;
        self.log_change("resources", &r.id, op, version)
    }
}
