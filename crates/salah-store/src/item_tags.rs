//! Task labels and the project defaults used for new tasks and project-only work.

use crate::{Store, WriteTx, codec};
use chrono::{DateTime, Utc};
use rusqlite::params;
use salah_domain::{error::Error, tracking::Tag};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct ReportTags {
    items: BTreeMap<String, Vec<Tag>>,
    projects: BTreeMap<String, Vec<Tag>>,
}

impl ReportTags {
    pub fn for_owner(&self, item: Option<&str>, project: Option<&str>) -> &[Tag] {
        let tags = match item {
            Some(id) => self.items.get(id),
            None => project.and_then(|id| self.projects.get(id)),
        };
        tags.map(Vec::as_slice).unwrap_or_default()
    }
}

impl Store {
    /// A task's live tag ids in tag-name order, like [`Store::report_tags`].
    pub fn item_tag_ids(&self, id: &str) -> Result<Vec<String>, Error> {
        self.all(
            "SELECT it.tag_id FROM item_tags it JOIN tags t ON t.id=it.tag_id
                  WHERE it.item_id=?1 AND t.deleted_at IS NULL ORDER BY t.name, t.id",
            [id],
            |r| r.get(0),
        )
    }

    pub fn project_default_tags(&self, id: &str) -> Result<Vec<String>, Error> {
        self.all(
            "SELECT pt.tag_id FROM project_tags pt JOIN tags t ON t.id=pt.tag_id
                  WHERE pt.project_id=?1 AND t.deleted_at IS NULL ORDER BY pt.tag_id",
            [id],
            |r| r.get(0),
        )
    }

    pub fn report_tags(&self) -> Result<ReportTags, Error> {
        let rows = self.all(
            "SELECT l.*, t.* FROM (
            SELECT 1 AS is_item, item_id AS owner, tag_id FROM item_tags UNION ALL
            SELECT 0, project_id, tag_id FROM project_tags
          ) l JOIN tags t ON t.id=l.tag_id WHERE t.deleted_at IS NULL ORDER BY t.name, t.id",
            [],
            |row| {
                Ok((
                    row.get::<_, bool>("is_item")?,
                    row.get::<_, String>("owner")?,
                    codec::tag(row)?,
                ))
            },
        )?;
        let mut tags = ReportTags::default();
        for (item, owner, tag) in rows {
            let owners = if item {
                &mut tags.items
            } else {
                &mut tags.projects
            };
            owners.entry(owner).or_default().push(tag);
        }
        Ok(tags)
    }
}

impl WriteTx<'_> {
    /// The caller writes the item version in the same transaction.
    pub fn set_item_tags(&self, id: &str, tags: &[String]) -> Result<(), Error> {
        self.execute("DELETE FROM item_tags WHERE item_id=?1", [id])?;
        for tag in tags {
            self.execute(
                "INSERT INTO item_tags(item_id,tag_id) VALUES (?1,?2)",
                params![id, tag],
            )?;
        }
        Ok(())
    }

    /// Takes a tag off every task, bumping each task's version (removed tasks
    /// too: their row changed, so the op is "update" like `next_version`).
    /// Returns the re-versioned item ids in id order.
    pub fn detach_item_tag(&self, id: &str, now: DateTime<Utc>) -> Result<Vec<String>, Error> {
        let mut changed = self.all(
            "UPDATE items SET version=version+1, updated_at=?2
            WHERE id IN (SELECT item_id FROM item_tags WHERE tag_id=?1)
            RETURNING id, version",
            params![id, codec::timestamp(now)],
            |r| Ok((r.get::<_, String>(0)?, r.get(1)?)),
        )?;
        changed.sort();
        for (item, version) in &changed {
            self.log_change("item", item, "update", *version)?;
        }
        self.execute("DELETE FROM item_tags WHERE tag_id=?1", [id])?;
        Ok(changed.into_iter().map(|(item, _)| item).collect())
    }
}
