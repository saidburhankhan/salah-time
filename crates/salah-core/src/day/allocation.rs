//! Allocate a session's recorded work over its tracking day's block windows.

use salah_domain::{
    day::BoundaryId,
    tracking::{DayBlock, Session},
};
use std::cmp::Reverse;

/// Positive whole minutes per block. Manual logs and zero-duration timers use
/// their stored block. Ticks and running timers have no recorded minutes.
pub fn block_minutes(session: &Session, blocks: &[DayBlock]) -> Vec<(BoundaryId, i64)> {
    let Some(minutes) = session.minutes else {
        return Vec::new();
    };
    let start_block = session.block.start;
    let (Some(start), Some(end)) = (session.started_at, session.ended_at) else {
        return vec![(start_block, minutes)];
    };
    let spans: Vec<_> = blocks
        .iter()
        .filter_map(|block| {
            let duration = end.min(block.ends_at) - start.max(block.starts_at);
            let millis = duration.num_milliseconds();
            (millis > 0).then_some((block.block, millis))
        })
        .collect();
    let total: i64 = spans.iter().map(|(_, span)| span).sum();
    if total == 0 {
        return vec![(start_block, minutes)];
    }

    // Apportion integer minutes by largest remainder. Resolve equal
    // remainders at the starting block first, then in prayer order.
    let mut shares: Vec<_> = spans
        .into_iter()
        .map(|(block, span)| {
            let scaled = i128::from(minutes) * i128::from(span);
            let total = i128::from(total);
            (block, (scaled / total) as i64, scaled % total)
        })
        .collect();
    let assigned: i64 = shares.iter().map(|(_, whole, _)| whole).sum();
    shares
        .sort_by_key(|(block, _, remainder)| (Reverse(*remainder), *block != start_block, *block));
    for (_, whole, _) in shares.iter_mut().take((minutes - assigned) as usize) {
        *whole += 1;
    }
    shares.sort_by_key(|(block, _, _)| *block);
    shares
        .into_iter()
        .filter(|(_, whole, _)| *whole > 0)
        .map(|(block, whole, _)| (block, whole))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{at, date};
    use BoundaryId::{Asr, Dhuhr, Fajr};
    use chrono::Duration;
    use salah_domain::{day::BlockKey, tracking::DayBlockSource};

    fn timer() -> Session {
        let start = at(20, 11, 0);
        Session {
            id: "timer".into(),
            item_id: None,
            project_id: None,
            epic_id: None,
            block: BlockKey {
                day: date(20),
                start: Fajr,
            },
            started_at: Some(start),
            ended_at: Some(start + Duration::hours(2)),
            minutes: Some(120),
            note: None,
            values: vec![],
            created_at: start,
            updated_at: start,
            deleted_at: None,
            version: 1,
        }
    }

    #[test]
    fn two_hours_across_three_blocks_scale_to_the_worked_override() {
        let mut session = timer();
        let start = session.started_at.unwrap();
        let blocks =
            [(Fajr, -200, 40), (Dhuhr, 40, 80), (Asr, 80, 240)].map(|(block, from, to)| DayBlock {
                day: date(20),
                block,
                starts_at: start + Duration::minutes(from),
                ends_at: start + Duration::minutes(to),
                source: DayBlockSource::Official,
            });
        assert_eq!(
            block_minutes(&session, &blocks),
            [(Fajr, 40), (Dhuhr, 40), (Asr, 40)]
        );
        session.minutes = Some(61);
        assert_eq!(
            block_minutes(&session, &blocks),
            [(Fajr, 21), (Dhuhr, 20), (Asr, 20)]
        );
        session.minutes = Some(1);
        assert_eq!(block_minutes(&session, &blocks), [(Fajr, 1)]);
        // After moving the start, Dhuhr and Asr have the largest fractions.
        session.started_at = Some(start + Duration::minutes(20));
        session.minutes = Some(2);
        assert_eq!(block_minutes(&session, &blocks), [(Dhuhr, 1), (Asr, 1)]);
    }

    #[test]
    fn instant_timers_and_manual_logs_stay_in_the_recorded_block() {
        let mut session = timer();
        session.ended_at = session.started_at;
        session.minutes = Some(1);
        assert_eq!(block_minutes(&session, &[]), [(Fajr, 1)]);
        session.started_at = None;
        session.ended_at = None;
        session.minutes = Some(30);
        assert_eq!(block_minutes(&session, &[]), [(Fajr, 30)]);
        session.minutes = None;
        assert!(block_minutes(&session, &[]).is_empty());
    }
}
