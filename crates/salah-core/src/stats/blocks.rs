//! Prayer-block attribution for reports. Boundaries are read once per timer day
//! in the same database snapshot as the sessions.

use super::table::{Bucket, Sample};
use crate::{Store, config::Config, day};
use chrono::NaiveDate;
use salah_domain::{day::BoundaryId, error::Error, tracking::DayBlock};
use salah_store::reports::ReportSession;
use std::collections::{BTreeMap, btree_map::Entry};

#[derive(Clone, Copy)]
pub struct Calendar<'a> {
    pub config: &'a Config,
    pub today: NaiveDate,
}

pub(super) type Windows = BTreeMap<NaiveDate, Vec<DayBlock>>;

impl Calendar<'_> {
    pub(super) fn windows(
        &self,
        store: &Store,
        sessions: &[ReportSession],
    ) -> Result<Windows, Error> {
        let mut windows = Windows::new();
        for row in sessions {
            let s = &row.session;
            if s.started_at.is_some()
                && let Entry::Vacant(entry) = windows.entry(s.block.day)
            {
                entry.insert(day::report_blocks(
                    store,
                    self.config,
                    s.block.day,
                    self.today,
                )?);
            }
        }
        Ok(windows)
    }
}

pub(super) fn buckets<'a>(sessions: &'a [ReportSession], windows: &Windows) -> Vec<Bucket<'a>> {
    let mut groups = BTreeMap::<BoundaryId, Vec<Sample<'a>>>::new();
    for row in sessions {
        let session = &row.session;
        if session.is_tick() {
            groups
                .entry(session.block.start)
                .or_default()
                .push(Sample::whole(session));
        } else {
            let blocks = windows
                .get(&session.block.day)
                .map(Vec::as_slice)
                .unwrap_or_default();
            for (block, minutes) in day::block_minutes(session, blocks) {
                groups.entry(block).or_default().push(Sample {
                    session,
                    minutes: Some(minutes),
                });
            }
        }
    }
    groups
        .into_iter()
        .map(|(block, sessions)| Bucket {
            key: block.as_str().into(),
            label: block.name().into(),
            color: None,
            from: None,
            to: None,
            sessions,
        })
        .collect()
}
