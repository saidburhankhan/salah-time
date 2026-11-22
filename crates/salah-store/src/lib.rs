//! SQLite persistence for Salah's v1 tracking model.
use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use salah_domain::{
    day::BlockKey,
    error::Error,
    item::{LegacyEntry, LegacyEntryKind, LegacyItem, LegacyMeasure, PrayerMark, Repeat, Weekdays},
    tracking::{
        DayBlock, DayPlanLine, Item, Metric, Project, ProjectMetric, Session, SessionValue, Tag,
    },
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    cell::{Cell, RefCell},
    ops::Deref,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const ATTEMPTS: u32 = 5;
const SCHEMA: &str = r#"
CREATE TABLE projects(id TEXT PRIMARY KEY,name TEXT NOT NULL,color TEXT NOT NULL,note TEXT,archived INTEGER NOT NULL CHECK(archived IN(0,1)),created_at TEXT NOT NULL,updated_at TEXT NOT NULL,deleted_at TEXT,version INTEGER NOT NULL CHECK(version>=1));
CREATE TABLE tags(id TEXT PRIMARY KEY,name TEXT NOT NULL UNIQUE,color TEXT NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL,deleted_at TEXT,version INTEGER NOT NULL CHECK(version>=1));
CREATE TABLE project_tags(project_id TEXT NOT NULL REFERENCES projects(id),tag_id TEXT NOT NULL REFERENCES tags(id),PRIMARY KEY(project_id,tag_id));
CREATE TABLE metrics(id TEXT PRIMARY KEY,name TEXT NOT NULL,unit TEXT NOT NULL,kind TEXT NOT NULL CHECK(kind IN('count','scale','check')),scale_max INTEGER,rollup TEXT NOT NULL CHECK(rollup IN('sum','avg')),builtin INTEGER NOT NULL CHECK(builtin IN(0,1)),archived INTEGER NOT NULL CHECK(archived IN(0,1)),created_at TEXT NOT NULL,updated_at TEXT NOT NULL,deleted_at TEXT,version INTEGER NOT NULL CHECK(version>=1),CHECK((kind='scale' AND scale_max>=1) OR (kind!='scale' AND scale_max IS NULL)));
CREATE TABLE project_metrics(project_id TEXT NOT NULL REFERENCES projects(id),metric_id TEXT NOT NULL REFERENCES metrics(id),required INTEGER NOT NULL CHECK(required IN(0,1)),position INTEGER NOT NULL,PRIMARY KEY(project_id,metric_id));
CREATE TABLE items(id TEXT PRIMARY KEY,title TEXT NOT NULL,project_id TEXT REFERENCES projects(id),block TEXT NOT NULL CHECK(block IN('fajr','sunrise','dhuhr','asr','maghrib','isha')),repeat TEXT NOT NULL CHECK(repeat IN('none','daily','weekly')),weekdays INTEGER,start_day TEXT NOT NULL,planned_minutes INTEGER CHECK(planned_minutes IS NULL OR planned_minutes>=0),position INTEGER NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL,deleted_at TEXT,version INTEGER NOT NULL CHECK(version>=1),CHECK((repeat='weekly' AND weekdays BETWEEN 1 AND 127) OR (repeat!='weekly' AND weekdays IS NULL)));
CREATE TABLE sessions(id TEXT PRIMARY KEY,item_id TEXT REFERENCES items(id),project_id TEXT REFERENCES projects(id),day TEXT NOT NULL,block TEXT NOT NULL CHECK(block IN('sleep','fajr','sunrise','dhuhr','asr','maghrib','isha')),started_at TEXT,ended_at TEXT,minutes INTEGER CHECK(minutes IS NULL OR minutes>=1),note TEXT,created_at TEXT NOT NULL,updated_at TEXT NOT NULL,deleted_at TEXT,version INTEGER NOT NULL CHECK(version>=1),CHECK(item_id IS NOT NULL OR project_id IS NOT NULL));
CREATE TABLE session_values(session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,metric_id TEXT NOT NULL REFERENCES metrics(id),value REAL NOT NULL,PRIMARY KEY(session_id,metric_id));
CREATE TABLE day_blocks(day TEXT NOT NULL,block TEXT NOT NULL CHECK(block IN('sleep','fajr','sunrise','dhuhr','asr','maghrib','isha')),starts_at TEXT NOT NULL,ends_at TEXT NOT NULL,source TEXT NOT NULL CHECK(source IN('official','calculated')),PRIMARY KEY(day,block));
CREATE TABLE day_plan(day TEXT NOT NULL,block TEXT NOT NULL CHECK(block IN('sleep','fajr','sunrise','dhuhr','asr','maghrib','isha')),item_id TEXT NOT NULL REFERENCES items(id),planned_minutes INTEGER CHECK(planned_minutes IS NULL OR planned_minutes>=0),PRIMARY KEY(day,block,item_id));
CREATE TABLE prayer_marks(id TEXT PRIMARY KEY,day TEXT NOT NULL,prayer TEXT NOT NULL,marked_at TEXT NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL,deleted_at TEXT,version INTEGER NOT NULL CHECK(version>=1));
CREATE TABLE meta(id INTEGER PRIMARY KEY CHECK(id=1),revision INTEGER NOT NULL);INSERT INTO meta VALUES(1,0);
CREATE TABLE changes(seq INTEGER PRIMARY KEY AUTOINCREMENT,revision INTEGER NOT NULL,entity TEXT NOT NULL,entity_id TEXT NOT NULL,op TEXT NOT NULL CHECK(op IN('insert','update','delete')),version INTEGER NOT NULL,at TEXT NOT NULL,op_id TEXT);
CREATE TABLE applied_ops(op_id TEXT PRIMARY KEY,revision INTEGER NOT NULL,result_json TEXT NOT NULL,at TEXT NOT NULL);
CREATE INDEX sessions_by_day ON sessions(day);CREATE INDEX sessions_by_project_day ON sessions(project_id,day);CREATE INDEX sessions_by_item ON sessions(item_id);CREATE INDEX session_values_by_metric ON session_values(metric_id);CREATE INDEX items_by_project ON items(project_id);CREATE INDEX prayer_marks_by_day ON prayer_marks(day);CREATE INDEX changes_by_revision ON changes(revision);CREATE INDEX changes_by_entity ON changes(entity,entity_id);
"#;
pub fn schema_version() -> i64 {
    1
}
pub fn default_path() -> PathBuf {
    if let Some(v) = std::env::var_os("SALAH_DATA").filter(|v| !v.is_empty()) {
        return v.into();
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|v| PathBuf::from(v).join(".local/share")))
        .unwrap_or_else(|| ".".into());
    base.join("salah/salah.db")
}
pub struct Store {
    conn: Connection,
}
fn se(e: rusqlite::Error) -> Error {
    let s = format!("store: {e}");
    if matches!(&e,rusqlite::Error::SqliteFailure(d,_) if matches!(d.code,rusqlite::ErrorCode::DatabaseBusy|rusqlite::ErrorCode::DatabaseLocked))
    {
        Error::Busy(s)
    } else {
        Error::Storage(s)
    }
}
fn tt(v: DateTime<Utc>) -> String {
    v.to_rfc3339_opts(SecondsFormat::Millis, true)
}
fn dt(v: NaiveDate) -> String {
    v.format("%Y-%m-%d").to_string()
}
fn pt(v: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(v)
        .map(|v| v.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
        })
}
fn pot(v: Option<String>) -> rusqlite::Result<Option<DateTime<Utc>>> {
    v.as_deref().map(pt).transpose()
}
fn pd(v: &str) -> rusqlite::Result<NaiveDate> {
    NaiveDate::parse_from_str(v, "%Y-%m-%d").map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
    })
}
fn et<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}
fn pe<T: DeserializeOwned>(v: &str) -> rusqlite::Result<T> {
    serde_json::from_value(serde_json::Value::String(v.into())).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
    })
}
fn item(r: &Row) -> rusqlite::Result<Item> {
    Ok(Item {
        id: r.get("id")?,
        title: r.get("title")?,
        project_id: r.get("project_id")?,
        block: pe(&r.get::<_, String>("block")?)?,
        repeat: pe::<Repeat>(&r.get::<_, String>("repeat")?)?,
        weekdays: r.get::<_, Option<u8>>("weekdays")?.map(Weekdays),
        day: pd(&r.get::<_, String>("start_day")?)?,
        planned_minutes: r.get("planned_minutes")?,
        position: r.get("position")?,
        created_at: pt(&r.get::<_, String>("created_at")?)?,
        updated_at: pt(&r.get::<_, String>("updated_at")?)?,
        deleted_at: pot(r.get("deleted_at")?)?,
        version: r.get("version")?,
    })
}
fn project(r: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get("id")?,
        name: r.get("name")?,
        color: r.get("color")?,
        note: r.get("note")?,
        archived: r.get("archived")?,
        tag_ids: vec![],
        created_at: pt(&r.get::<_, String>("created_at")?)?,
        updated_at: pt(&r.get::<_, String>("updated_at")?)?,
        deleted_at: pot(r.get("deleted_at")?)?,
        version: r.get("version")?,
    })
}
fn tag(r: &Row) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: r.get("id")?,
        name: r.get("name")?,
        color: r.get("color")?,
        created_at: pt(&r.get::<_, String>("created_at")?)?,
        updated_at: pt(&r.get::<_, String>("updated_at")?)?,
        deleted_at: pot(r.get("deleted_at")?)?,
        version: r.get("version")?,
    })
}
fn metric(r: &Row) -> rusqlite::Result<Metric> {
    Ok(Metric {
        id: r.get("id")?,
        name: r.get("name")?,
        unit: r.get("unit")?,
        kind: pe(&r.get::<_, String>("kind")?)?,
        scale_max: r.get("scale_max")?,
        rollup: pe(&r.get::<_, String>("rollup")?)?,
        builtin: r.get("builtin")?,
        archived: r.get("archived")?,
        created_at: pt(&r.get::<_, String>("created_at")?)?,
        updated_at: pt(&r.get::<_, String>("updated_at")?)?,
        deleted_at: pot(r.get("deleted_at")?)?,
        version: r.get("version")?,
    })
}
fn mark(r: &Row) -> rusqlite::Result<PrayerMark> {
    Ok(PrayerMark {
        id: r.get("id")?,
        day: pd(&r.get::<_, String>("day")?)?,
        prayer: pe(&r.get::<_, String>("prayer")?)?,
        marked_at: pt(&r.get::<_, String>("marked_at")?)?,
        created_at: pt(&r.get::<_, String>("created_at")?)?,
        updated_at: pt(&r.get::<_, String>("updated_at")?)?,
        deleted_at: pot(r.get("deleted_at")?)?,
        version: r.get("version")?,
    })
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub seq: i64,
    pub revision: i64,
    pub entity: String,
    pub entity_id: String,
    pub op: String,
    pub version: i64,
    pub at: DateTime<Utc>,
    pub op_id: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedOp {
    pub revision: i64,
    pub result_json: String,
    pub at: DateTime<Utc>,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self, Error> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| Error::Storage(format!("store: {e}")))?
        }
        if path.exists() {
            let c = Connection::open(path).map_err(se)?;
            let v: i64 = c
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .map_err(se)?;
            let is_v1: bool = c
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='metrics')",
                    [],
                    |r| r.get(0),
                )
                .map_err(se)?;
            drop(c);
            if v > 0 && !is_v1 {
                let n = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let name = path
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or("salah.db");
                std::fs::rename(path, path.with_file_name(format!("{name}.old-{n}")))
                    .map_err(|e| Error::Storage(format!("store: {e}")))?
            }
        }
        Self::init(Connection::open(path).map_err(se)?)
    }
    pub fn open_in_memory() -> Result<Self, Error> {
        Self::init(Connection::open_in_memory().map_err(se)?)
    }
    fn init(conn: Connection) -> Result<Self, Error> {
        conn.busy_timeout(Duration::from_secs(3)).map_err(se)?;
        conn.pragma_update(None, "foreign_keys", "ON").map_err(se)?;
        conn.execute_batch("BEGIN IMMEDIATE").map_err(se)?;
        let v: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(se)?;
        if v == 0 {
            conn.execute_batch(SCHEMA).map_err(se)?;
            conn.pragma_update(None, "user_version", 1).map_err(se)?;
            let n = tt(Utc::now());
            for (id, name) in [("focus", "Focus"), ("quality", "Quality")] {
                conn.execute(
                    "INSERT INTO metrics VALUES(?1,?2,'','scale',5,'avg',1,0,?3,?3,NULL,1)",
                    params![id, name, n],
                )
                .map_err(se)?;
            }
        } else if v != 1 {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(Error::Storage(format!("store: unsupported schema {v}")));
        }
        conn.execute_batch("COMMIT").map_err(se)?;
        Ok(Self { conn })
    }
    pub fn version(&self) -> Result<i64, Error> {
        self.conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(se)
    }
    pub fn revision(&self) -> Result<i64, Error> {
        self.conn
            .query_row("SELECT revision FROM meta", [], |r| r.get(0))
            .map_err(se)
    }
    pub fn data_version(&self) -> Result<i64, Error> {
        self.conn
            .pragma_query_value(None, "data_version", |r| r.get(0))
            .map_err(se)
    }
    pub fn foreign_keys_enabled(&self) -> Result<bool, Error> {
        self.conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .map_err(se)
    }
    fn q<T, P: rusqlite::Params>(
        &self,
        s: &str,
        p: P,
        f: impl FnMut(&Row) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, Error> {
        let mut q = self.conn.prepare(s).map_err(se)?;
        q.query_map(p, f)
            .map_err(se)?
            .collect::<rusqlite::Result<_>>()
            .map_err(se)
    }
    fn one<T>(
        &self,
        s: &str,
        id: &str,
        f: fn(&Row) -> rusqlite::Result<T>,
    ) -> Result<Option<T>, Error> {
        self.conn.query_row(s, [id], f).optional().map_err(se)
    }
    pub fn last_change_seq(&self) -> Result<i64, Error> {
        self.conn
            .query_row("SELECT COALESCE(MAX(seq),0) FROM changes", [], |r| r.get(0))
            .map_err(se)
    }
    pub fn changes_since(&self, a: i64, l: usize) -> Result<Vec<Change>, Error> {
        self.q(
            "SELECT * FROM changes WHERE seq>?1 ORDER BY seq LIMIT ?2",
            params![a, l.min(10000) as i64],
            |r| {
                Ok(Change {
                    seq: r.get(0)?,
                    revision: r.get(1)?,
                    entity: r.get(2)?,
                    entity_id: r.get(3)?,
                    op: r.get(4)?,
                    version: r.get(5)?,
                    at: pt(&r.get::<_, String>(6)?)?,
                    op_id: r.get(7)?,
                })
            },
        )
    }
    pub fn applied_op(&self, id: &str) -> Result<Option<AppliedOp>, Error> {
        self.conn
            .query_row(
                "SELECT revision,result_json,at FROM applied_ops WHERE op_id=?1",
                [id],
                |r| {
                    Ok(AppliedOp {
                        revision: r.get(0)?,
                        result_json: r.get(1)?,
                        at: pt(&r.get::<_, String>(2)?)?,
                    })
                },
            )
            .optional()
            .map_err(se)
    }
    pub fn write<T>(
        &mut self,
        mut f: impl FnMut(&WriteTx<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        for a in 0..ATTEMPTS {
            self.conn.execute_batch("BEGIN IMMEDIATE").map_err(se)?;
            let tx = WriteTx {
                store: self,
                revision: Cell::new(None),
                op_id: RefCell::new(None),
            };
            match f(&tx) {
                Ok(v) => {
                    self.conn.execute_batch("COMMIT").map_err(se)?;
                    return Ok(v);
                }
                Err(e) => {
                    let _ = self.conn.execute_batch("ROLLBACK");
                    if matches!(e, Error::Busy(_)) && a + 1 < ATTEMPTS {
                        std::thread::sleep(Duration::from_millis(5 << a));
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        unreachable!()
    }
    fn values(&self, id: &str) -> Result<Vec<SessionValue>, Error> {
        self.q(
            "SELECT metric_id,value FROM session_values WHERE session_id=?1 ORDER BY metric_id",
            [id],
            |r| {
                Ok(SessionValue {
                    metric_id: r.get(0)?,
                    value: r.get(1)?,
                })
            },
        )
    }
    fn sr(&self, r: &Row) -> rusqlite::Result<Session> {
        let id: String = r.get("id")?;
        Ok(Session {
            id: id.clone(),
            item_id: r.get("item_id")?,
            project_id: r.get("project_id")?,
            block: BlockKey {
                day: pd(&r.get::<_, String>("day")?)?,
                start: pe(&r.get::<_, String>("block")?)?,
            },
            started_at: pot(r.get("started_at")?)?,
            ended_at: pot(r.get("ended_at")?)?,
            minutes: r.get("minutes")?,
            note: r.get("note")?,
            values: self
                .values(&id)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
            created_at: pt(&r.get::<_, String>("created_at")?)?,
            updated_at: pt(&r.get::<_, String>("updated_at")?)?,
            deleted_at: pot(r.get("deleted_at")?)?,
            version: r.get("version")?,
        })
    }
    pub fn project(&self, id: &str) -> Result<Option<Project>, Error> {
        let mut p = self.one("SELECT * FROM projects WHERE id=?1", id, project)?;
        if let Some(p) = &mut p {
            p.tag_ids = self.q(
                "SELECT tag_id FROM project_tags WHERE project_id=?1 ORDER BY tag_id",
                [id],
                |r| r.get(0),
            )?
        }
        Ok(p)
    }
    pub fn projects(&self) -> Result<Vec<Project>, Error> {
        let mut p = self.q("SELECT * FROM projects ORDER BY created_at,id", [], project)?;
        for v in &mut p {
            v.tag_ids = self.q(
                "SELECT tag_id FROM project_tags WHERE project_id=?1 ORDER BY tag_id",
                [&v.id],
                |r| r.get(0),
            )?
        }
        Ok(p)
    }
    pub fn tag(&self, id: &str) -> Result<Option<Tag>, Error> {
        self.one("SELECT * FROM tags WHERE id=?1", id, tag)
    }
    pub fn tags(&self) -> Result<Vec<Tag>, Error> {
        self.q("SELECT * FROM tags ORDER BY name,id", [], tag)
    }
    pub fn metric(&self, id: &str) -> Result<Option<Metric>, Error> {
        self.one("SELECT * FROM metrics WHERE id=?1", id, metric)
    }
    pub fn metrics(&self) -> Result<Vec<Metric>, Error> {
        self.q(
            "SELECT * FROM metrics ORDER BY builtin DESC,name,id",
            [],
            metric,
        )
    }
    pub fn project_metrics(&self, id: &str) -> Result<Vec<ProjectMetric>, Error> {
        self.q("SELECT project_id,metric_id,required,position FROM project_metrics WHERE project_id=?1 ORDER BY position",[id],|r|Ok(ProjectMetric{project_id:r.get(0)?,metric_id:r.get(1)?,required:r.get(2)?,position:r.get(3)?}))
    }
    pub fn item(&self, id: &str) -> Result<Option<Item>, Error> {
        self.one("SELECT * FROM items WHERE id=?1", id, item)
    }
    pub fn items(&self) -> Result<Vec<Item>, Error> {
        self.q("SELECT * FROM items ORDER BY created_at,id", [], item)
    }
    pub fn session(&self, id: &str) -> Result<Option<Session>, Error> {
        self.conn
            .query_row("SELECT * FROM sessions WHERE id=?1", [id], |r| self.sr(r))
            .optional()
            .map_err(se)
    }
    pub fn sessions(&self) -> Result<Vec<Session>, Error> {
        self.q("SELECT * FROM sessions ORDER BY created_at,id", [], |r| {
            self.sr(r)
        })
    }
    pub fn sessions_between(&self, a: NaiveDate, b: NaiveDate) -> Result<Vec<Session>, Error> {
        self.q(
            "SELECT * FROM sessions WHERE day BETWEEN ?1 AND ?2 ORDER BY day,created_at",
            [dt(a), dt(b)],
            |r| self.sr(r),
        )
    }
    pub fn running_sessions(&self) -> Result<Vec<Session>, Error> {
        self.q("SELECT * FROM sessions WHERE started_at IS NOT NULL AND ended_at IS NULL AND deleted_at IS NULL",[],|r|self.sr(r))
    }
    pub fn day_blocks(&self, d: NaiveDate) -> Result<Vec<DayBlock>, Error> {
        self.q(
            "SELECT * FROM day_blocks WHERE day=?1 ORDER BY starts_at",
            [dt(d)],
            |r| {
                Ok(DayBlock {
                    day: pd(&r.get::<_, String>(0)?)?,
                    block: pe(&r.get::<_, String>(1)?)?,
                    starts_at: pt(&r.get::<_, String>(2)?)?,
                    ends_at: pt(&r.get::<_, String>(3)?)?,
                    source: pe(&r.get::<_, String>(4)?)?,
                })
            },
        )
    }
    pub fn day_plan(&self, d: NaiveDate) -> Result<Vec<DayPlanLine>, Error> {
        self.q(
            "SELECT * FROM day_plan WHERE day=?1 ORDER BY block,item_id",
            [dt(d)],
            |r| {
                Ok(DayPlanLine {
                    day: pd(&r.get::<_, String>(0)?)?,
                    block: pe(&r.get::<_, String>(1)?)?,
                    item_id: r.get(2)?,
                    planned_minutes: r.get(3)?,
                })
            },
        )
    }
    pub fn mark(&self, id: &str) -> Result<Option<PrayerMark>, Error> {
        self.one("SELECT * FROM prayer_marks WHERE id=?1", id, mark)
    }
    pub fn marks(&self) -> Result<Vec<PrayerMark>, Error> {
        self.q(
            "SELECT * FROM prayer_marks ORDER BY created_at,id",
            [],
            mark,
        )
    }
    pub fn marks_between(&self, a: NaiveDate, b: NaiveDate) -> Result<Vec<PrayerMark>, Error> {
        self.q(
            "SELECT * FROM prayer_marks WHERE day BETWEEN ?1 AND ?2 ORDER BY day,id",
            [dt(a), dt(b)],
            mark,
        )
    }
    pub fn legacy_item(&self, id: &str) -> Result<Option<LegacyItem>, Error> {
        self.item(id)?.map(new_to_legacy_item).transpose()
    }
    pub fn legacy_items(&self) -> Result<Vec<LegacyItem>, Error> {
        self.items()?.into_iter().map(new_to_legacy_item).collect()
    }
    pub fn entry(&self, id: &str) -> Result<Option<LegacyEntry>, Error> {
        self.session(id)?.map(session_to_entry).transpose()
    }
    pub fn entries(&self) -> Result<Vec<LegacyEntry>, Error> {
        self.sessions()?.into_iter().map(session_to_entry).collect()
    }
    pub fn entries_between(&self, a: NaiveDate, b: NaiveDate) -> Result<Vec<LegacyEntry>, Error> {
        self.sessions_between(a, b)?
            .into_iter()
            .map(session_to_entry)
            .collect()
    }
    pub fn running_timers(&self) -> Result<Vec<LegacyEntry>, Error> {
        self.running_sessions()?
            .into_iter()
            .map(session_to_entry)
            .collect()
    }
}

fn new_to_legacy_item(i: Item) -> Result<LegacyItem, Error> {
    Ok(LegacyItem {
        id: i.id,
        title: i.title,
        block: i.block,
        repeat: i.repeat,
        weekdays: i.weekdays,
        measure: if i.planned_minutes.is_some() {
            LegacyMeasure::Minutes
        } else {
            LegacyMeasure::Check
        },
        target: i.planned_minutes,
        position: i.position,
        day: i.day,
        created_at: i.created_at,
        updated_at: i.updated_at,
        deleted_at: i.deleted_at,
        version: i.version,
    })
}
fn legacy_to_new_item(i: &LegacyItem) -> Item {
    Item {
        id: i.id.clone(),
        title: i.title.clone(),
        project_id: None,
        block: i.block,
        repeat: i.repeat,
        weekdays: i.weekdays,
        day: i.day,
        planned_minutes: i.target,
        position: i.position,
        created_at: i.created_at,
        updated_at: i.updated_at,
        deleted_at: i.deleted_at,
        version: i.version,
    }
}
fn session_to_entry(s: Session) -> Result<LegacyEntry, Error> {
    let kind = if s.started_at.is_some() {
        LegacyEntryKind::Timer
    } else if s.minutes.is_some() {
        LegacyEntryKind::Minutes
    } else {
        LegacyEntryKind::Done
    };
    Ok(LegacyEntry {
        id: s.id,
        item_id: s
            .item_id
            .unwrap_or_else(|| s.project_id.unwrap_or_default()),
        block: s.block,
        kind,
        value: s.minutes,
        started_at: s.started_at,
        ended_at: s.ended_at,
        created_at: s.created_at,
        updated_at: s.updated_at,
        deleted_at: s.deleted_at,
        version: s.version,
    })
}
fn entry_to_session(e: &LegacyEntry, project_id: Option<String>) -> Session {
    Session {
        id: e.id.clone(),
        item_id: Some(e.item_id.clone()),
        project_id,
        block: e.block,
        started_at: e.started_at,
        ended_at: e.ended_at,
        minutes: match e.kind {
            LegacyEntryKind::Done => None,
            _ => e.value.map(|v| v.abs().max(1)),
        },
        note: None,
        values: vec![],
        created_at: e.created_at,
        updated_at: e.updated_at,
        deleted_at: e.deleted_at,
        version: e.version,
    }
}
pub struct WriteTx<'a> {
    store: &'a Store,
    revision: Cell<Option<i64>>,
    op_id: RefCell<Option<String>>,
}
impl Deref for WriteTx<'_> {
    type Target = Store;
    fn deref(&self) -> &Store {
        self.store
    }
}
impl WriteTx<'_> {
    pub fn set_op_id(&self, v: Option<&str>) {
        *self.op_id.borrow_mut() = v.map(str::to_owned)
    }
    fn rev(&self) -> Result<i64, Error> {
        if let Some(v) = self.revision.get() {
            Ok(v)
        } else {
            let v = self
                .store
                .conn
                .query_row(
                    "UPDATE meta SET revision=revision+1 RETURNING revision",
                    [],
                    |r| r.get(0),
                )
                .map_err(se)?;
            self.revision.set(Some(v));
            Ok(v)
        }
    }
    pub fn record_applied_op(&self, id: &str, json: &str) -> Result<(), Error> {
        let v = self
            .revision
            .get()
            .map_or_else(|| self.store.revision(), Ok)?;
        self.store
            .conn
            .execute(
                "INSERT INTO applied_ops VALUES(?1,?2,?3,?4) ON CONFLICT DO NOTHING",
                params![id, v, json, tt(Utc::now())],
            )
            .map(|_| ())
            .map_err(se)
    }
    fn vf(
        &self,
        table: &str,
        id: &str,
        del: Option<DateTime<Utc>>,
        explicit: Option<i64>,
    ) -> Result<(i64, &'static str), Error> {
        let old: Option<(i64, Option<String>)> = self
            .store
            .conn
            .query_row(
                &format!("SELECT version,deleted_at FROM {table} WHERE id=?1"),
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(se)?;
        Ok(match old {
            None => (explicit.unwrap_or(1).max(1), "insert"),
            Some((v, d)) => (
                explicit.map_or(v + 1, |x| x.max(v + 1)),
                if del.is_some() && d.is_none() {
                    "delete"
                } else {
                    "update"
                },
            ),
        })
    }
    fn done(&self, e: &str, id: &str, op: &str, v: i64) -> Result<i64, Error> {
        let r = self.rev()?;
        self.store.conn.execute("INSERT INTO changes(revision,entity,entity_id,op,version,at,op_id) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![r,e,id,op,v,tt(Utc::now()),self.op_id.borrow().as_deref()]).map_err(se)?;
        Ok(v)
    }
    pub fn put_project(&self, p: &Project) -> Result<i64, Error> {
        let (v, o) = self.vf("projects", &p.id, p.deleted_at, None)?;
        self.store.conn.execute("INSERT INTO projects VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO UPDATE SET name=excluded.name,color=excluded.color,note=excluded.note,archived=excluded.archived,updated_at=excluded.updated_at,deleted_at=excluded.deleted_at,version=excluded.version",params![p.id,p.name,p.color,p.note,p.archived,tt(p.created_at),tt(p.updated_at),p.deleted_at.map(tt),v]).map_err(se)?;
        self.store
            .conn
            .execute("DELETE FROM project_tags WHERE project_id=?1", [&p.id])
            .map_err(se)?;
        for t in &p.tag_ids {
            self.store
                .conn
                .execute("INSERT INTO project_tags VALUES(?1,?2)", params![p.id, t])
                .map_err(se)?;
        }
        self.done("project", &p.id, o, v)
    }
    pub fn put_tag(&self, t: &Tag) -> Result<i64, Error> {
        let (v, o) = self.vf("tags", &t.id, t.deleted_at, None)?;
        self.store.conn.execute("INSERT INTO tags VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET name=excluded.name,color=excluded.color,updated_at=excluded.updated_at,deleted_at=excluded.deleted_at,version=excluded.version",params![t.id,t.name,t.color,tt(t.created_at),tt(t.updated_at),t.deleted_at.map(tt),v]).map_err(se)?;
        self.done("tag", &t.id, o, v)
    }
    pub fn put_metric(&self, m: &Metric) -> Result<i64, Error> {
        let (v, o) = self.vf("metrics", &m.id, m.deleted_at, None)?;
        self.store.conn.execute("INSERT INTO metrics VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(id) DO UPDATE SET name=excluded.name,unit=excluded.unit,kind=excluded.kind,scale_max=excluded.scale_max,rollup=excluded.rollup,builtin=excluded.builtin,archived=excluded.archived,updated_at=excluded.updated_at,deleted_at=excluded.deleted_at,version=excluded.version",params![m.id,m.name,m.unit,et(&m.kind),m.scale_max,et(&m.rollup),m.builtin,m.archived,tt(m.created_at),tt(m.updated_at),m.deleted_at.map(tt),v]).map_err(se)?;
        self.done("metric", &m.id, o, v)
    }
    pub fn set_project_metrics(&self, id: &str, rows: &[ProjectMetric]) -> Result<(), Error> {
        self.store
            .conn
            .execute("DELETE FROM project_metrics WHERE project_id=?1", [id])
            .map_err(se)?;
        for r in rows {
            self.store
                .conn
                .execute(
                    "INSERT INTO project_metrics VALUES(?1,?2,?3,?4)",
                    params![r.project_id, r.metric_id, r.required, r.position],
                )
                .map_err(se)?;
        }
        Ok(())
    }
    fn wi(&self, i: &Item, x: Option<i64>) -> Result<i64, Error> {
        let (v, o) = self.vf("items", &i.id, i.deleted_at, x)?;
        self.store.conn.execute("INSERT INTO items VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(id) DO UPDATE SET title=excluded.title,project_id=excluded.project_id,block=excluded.block,repeat=excluded.repeat,weekdays=excluded.weekdays,start_day=excluded.start_day,planned_minutes=excluded.planned_minutes,position=excluded.position,updated_at=excluded.updated_at,deleted_at=excluded.deleted_at,version=excluded.version",params![i.id,i.title,i.project_id,et(&i.block),et(&i.repeat),i.weekdays.map(|w|w.0),dt(i.day),i.planned_minutes,i.position,tt(i.created_at),tt(i.updated_at),i.deleted_at.map(tt),v]).map_err(se)?;
        self.done("item", &i.id, o, v)
    }
    pub fn put_item(&self, i: &Item) -> Result<i64, Error> {
        self.wi(i, None)
    }
    pub fn put_item_versioned(&self, i: &Item, v: i64) -> Result<i64, Error> {
        self.wi(i, Some(v))
    }
    fn ws(&self, s: &Session, x: Option<i64>) -> Result<i64, Error> {
        let (v, o) = self.vf("sessions", &s.id, s.deleted_at, x)?;
        self.store.conn.execute("INSERT INTO sessions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(id) DO UPDATE SET item_id=excluded.item_id,project_id=excluded.project_id,day=excluded.day,block=excluded.block,started_at=excluded.started_at,ended_at=excluded.ended_at,minutes=excluded.minutes,note=excluded.note,updated_at=excluded.updated_at,deleted_at=excluded.deleted_at,version=excluded.version",params![s.id,s.item_id,s.project_id,dt(s.block.day),et(&s.block.start),s.started_at.map(tt),s.ended_at.map(tt),s.minutes,s.note,tt(s.created_at),tt(s.updated_at),s.deleted_at.map(tt),v]).map_err(se)?;
        self.store
            .conn
            .execute("DELETE FROM session_values WHERE session_id=?1", [&s.id])
            .map_err(se)?;
        for x in &s.values {
            self.store
                .conn
                .execute(
                    "INSERT INTO session_values VALUES(?1,?2,?3)",
                    params![s.id, x.metric_id, x.value],
                )
                .map_err(se)?;
        }
        self.done("session", &s.id, o, v)
    }
    pub fn put_session(&self, s: &Session) -> Result<i64, Error> {
        self.ws(s, None)
    }
    pub fn put_session_versioned(&self, s: &Session, v: i64) -> Result<i64, Error> {
        self.ws(s, Some(v))
    }
    pub fn replace_day_blocks(&self, d: NaiveDate, v: &[DayBlock]) -> Result<(), Error> {
        self.store
            .conn
            .execute("DELETE FROM day_blocks WHERE day=?1", [dt(d)])
            .map_err(se)?;
        for x in v {
            self.store
                .conn
                .execute(
                    "INSERT INTO day_blocks VALUES(?1,?2,?3,?4,?5)",
                    params![
                        dt(x.day),
                        et(&x.block),
                        tt(x.starts_at),
                        tt(x.ends_at),
                        et(&x.source)
                    ],
                )
                .map_err(se)?;
        }
        Ok(())
    }
    pub fn replace_day_plan(&self, d: NaiveDate, v: &[DayPlanLine]) -> Result<(), Error> {
        self.store
            .conn
            .execute("DELETE FROM day_plan WHERE day=?1", [dt(d)])
            .map_err(se)?;
        for x in v {
            self.store
                .conn
                .execute(
                    "INSERT INTO day_plan VALUES(?1,?2,?3,?4)",
                    params![dt(x.day), et(&x.block), x.item_id, x.planned_minutes],
                )
                .map_err(se)?;
        }
        Ok(())
    }
    fn wm(&self, m: &PrayerMark, x: Option<i64>) -> Result<i64, Error> {
        let (v, o) = self.vf("prayer_marks", &m.id, m.deleted_at, x)?;
        self.store.conn.execute("INSERT INTO prayer_marks VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET day=excluded.day,prayer=excluded.prayer,marked_at=excluded.marked_at,updated_at=excluded.updated_at,deleted_at=excluded.deleted_at,version=excluded.version",params![m.id,dt(m.day),et(&m.prayer),tt(m.marked_at),tt(m.created_at),tt(m.updated_at),m.deleted_at.map(tt),v]).map_err(se)?;
        self.done("prayer_mark", &m.id, o, v)
    }
    pub fn put_mark(&self, m: &PrayerMark) -> Result<i64, Error> {
        self.wm(m, None)
    }
    pub fn put_mark_versioned(&self, m: &PrayerMark, v: i64) -> Result<i64, Error> {
        self.wm(m, Some(v))
    }
    pub fn put_legacy_item(&self, i: &LegacyItem) -> Result<i64, Error> {
        self.put_item(&legacy_to_new_item(i))
    }
    pub fn put_legacy_item_versioned(&self, i: &LegacyItem, v: i64) -> Result<i64, Error> {
        self.put_item_versioned(&legacy_to_new_item(i), v)
    }
    pub fn put_entry(&self, e: &LegacyEntry) -> Result<i64, Error> {
        let project = self.item(&e.item_id)?.and_then(|i| i.project_id);
        self.put_session(&entry_to_session(e, project))
    }
    pub fn put_entry_versioned(&self, e: &LegacyEntry, v: i64) -> Result<i64, Error> {
        let project = self.item(&e.item_id)?.and_then(|i| i.project_id);
        self.put_session_versioned(&entry_to_session(e, project), v)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use salah_domain::day::BoundaryId;
    fn t(h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2031, 5, 14, h, 0, 0).unwrap()
    }
    fn d() -> NaiveDate {
        NaiveDate::from_ymd_opt(2031, 5, 14).unwrap()
    }
    fn p() -> Project {
        Project {
            id: "p".into(),
            name: "P".into(),
            color: "#fff".into(),
            note: None,
            archived: false,
            tag_ids: vec![],
            created_at: t(1),
            updated_at: t(1),
            deleted_at: None,
            version: 1,
        }
    }
    fn i() -> Item {
        Item {
            id: "i".into(),
            title: "I".into(),
            project_id: Some("p".into()),
            block: BoundaryId::Fajr,
            repeat: Repeat::Daily,
            weekdays: None,
            day: d(),
            planned_minutes: Some(30),
            position: 0,
            created_at: t(1),
            updated_at: t(1),
            deleted_at: None,
            version: 1,
        }
    }
    #[test]
    fn schema_rows() {
        let mut s = Store::open_in_memory().unwrap();
        assert!(s.foreign_keys_enabled().unwrap());
        assert_eq!(s.metrics().unwrap().len(), 2);
        s.write(|x| {
            x.put_project(&p())?;
            x.put_item(&i())
        })
        .unwrap();
        assert_eq!(s.item("i").unwrap(), Some(i()));
        assert_eq!(s.revision().unwrap(), 1)
    }
    #[test]
    fn checks() {
        let mut s = Store::open_in_memory().unwrap();
        s.write(|x| x.put_project(&p())).unwrap();
        let mut x = i();
        x.planned_minutes = Some(-1);
        assert!(s.write(|q| q.put_item(&x)).is_err())
    }
    #[test]
    fn old_database_is_renamed_aside() {
        let root = std::env::temp_dir().join(format!("salah-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("salah.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch("CREATE TABLE items(id TEXT); PRAGMA user_version=3;")
            .unwrap();
        drop(old);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.version().unwrap(), 1);
        assert!(std::fs::read_dir(&root).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("salah.db.old-")
        }));
        std::fs::remove_dir_all(root).unwrap();
    }
}
