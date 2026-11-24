use crate::{Store, WriteTx, codec};
use chrono::NaiveDate;
use rusqlite::params;
use salah_domain::{
    error::Error,
    review::{Reflection, validate_week},
};

impl Store {
    pub fn reflection(&self, week: NaiveDate) -> Result<Option<Reflection>, Error> {
        self.one(
            "SELECT * FROM reviews WHERE week=?1 AND deleted_at IS NULL",
            [codec::day(week)],
            |r| {
                Ok(Reflection {
                    id: r.get("id")?,
                    week: codec::get_day(r, "week")?,
                    note: r.get("note")?,
                    created_at: codec::get_timestamp(r, "created_at")?,
                    updated_at: codec::get_timestamp(r, "updated_at")?,
                    deleted_at: codec::get_optional_timestamp(r, "deleted_at")?,
                    version: r.get("version")?,
                })
            },
        )
    }

    pub fn open_backlog_count(&self) -> Result<i64, Error> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM items WHERE deleted_at IS NULL AND block IS NULL",
                [],
                |r| r.get(0),
            )
            .map_err(codec::storage_error)
    }
}

impl WriteTx<'_> {
    pub fn put_reflection(&self, row: &mut Reflection) -> Result<(), Error> {
        validate_week(row.week)?;
        if row.note.trim().is_empty() || row.note.contains('\0') || row.note.chars().count() > 5000
        {
            return Err(Error::validation(
                "note",
                "use 1 to 5000 characters without NUL",
            ));
        }
        let (version, op) = self.next_version("reviews", &row.id, row.deleted_at)?;
        self.execute("INSERT INTO reviews (id,week,note,created_at,updated_at,deleted_at,version)
            VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET
            note=excluded.note, updated_at=excluded.updated_at, deleted_at=excluded.deleted_at, version=excluded.version",
            params![row.id, codec::day(row.week),row.note,codec::timestamp(row.created_at),
            codec::timestamp(row.updated_at),row.deleted_at.map(codec::timestamp),version])?;
        self.log_change("reviews", &row.id, op, version)?;
        row.version = version;
        Ok(())
    }
}
