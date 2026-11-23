use crate::{Store, codec};
use salah_domain::{error::Error, resource::ResourceCounts};
use std::collections::BTreeMap;

impl Store {
    /// Live resource counts per item that has any.
    pub fn item_resource_counts(&self) -> Result<BTreeMap<String, ResourceCounts>, Error> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT item_id, kind, COUNT(*) FROM resources
                 WHERE item_id IS NOT NULL AND deleted_at IS NULL GROUP BY item_id, kind",
            )
            .map_err(codec::storage_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(codec::storage_error)?;
        let mut counts = BTreeMap::<String, ResourceCounts>::new();
        for row in rows {
            let (item_id, kind, n) = row.map_err(codec::storage_error)?;
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
