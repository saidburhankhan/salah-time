//! Prayer-block attribution for reports. Boundaries are read once per timer day
//! in the same database snapshot as the sessions.

use super::{
    BlockStat, aggregate,
    table::{Bucket, Sample},
};
use crate::{Store, config::Config, day};
use chrono::NaiveDate;
use salah_domain::{
    day::BoundaryId,
    error::Error,
    tracking::{DayBlock, Rollup, Session},
};
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

/// Group the same block samples for stats.get, stats.table and stats.series.
fn samples<'a, 'b>(
    sessions: impl Iterator<Item = (&'a Session, &'b [DayBlock])>,
) -> BTreeMap<BoundaryId, Vec<Sample<'a>>> {
    let mut groups = BTreeMap::<BoundaryId, Vec<Sample<'a>>>::new();
    for (session, blocks) in sessions {
        if session.is_tick() {
            groups
                .entry(session.block.start)
                .or_default()
                .push(Sample::whole(session));
        } else {
            for (block, minutes) in day::block_minutes(session, blocks) {
                groups.entry(block).or_default().push(Sample {
                    session,
                    minutes: Some(minutes),
                });
            }
        }
    }
    groups
}

pub(super) fn summary(sessions: &[Session], days: &[(NaiveDate, day::Snapshot)]) -> Vec<BlockStat> {
    let windows: BTreeMap<_, _> = days
        .iter()
        .map(|(day, snapshot)| (*day, snapshot.blocks.as_slice()))
        .collect();
    let groups = samples(sessions.iter().map(|s| (s, windows[&s.block.day])));
    BoundaryId::AWAKE
        .into_iter()
        .map(|block| {
            let inside = groups.get(&block).map(Vec::as_slice).unwrap_or_default();
            BlockStat {
                block,
                minutes: inside.iter().filter_map(|s| s.minutes).sum(),
                capacity_minutes: days
                    .iter()
                    .flat_map(|(_, snapshot)| &snapshot.blocks)
                    .filter(|b| b.block == block)
                    .map(day::block_capacity)
                    .sum(),
                avg_focus: aggregate(
                    Rollup::Avg,
                    inside.iter().filter_map(|s| s.metric(Rollup::Avg, "focus")),
                ),
            }
        })
        .collect()
}

pub(super) fn buckets<'a>(sessions: &'a [ReportSession], windows: &Windows) -> Vec<Bucket<'a>> {
    let groups = samples(sessions.iter().map(|row| {
        let session = &row.session;
        let blocks = windows
            .get(&session.block.day)
            .map(Vec::as_slice)
            .unwrap_or_default();
        (session, blocks)
    }));
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
