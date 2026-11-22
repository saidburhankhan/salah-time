use super::views::history;
use crate::config::Config;
use crate::day::BoundaryId;
use crate::error::Error;
use crate::item::{Item, Measure, PrayerMark, Repeat, State, Weekdays};
use crate::store::Store;
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct DayStat {
    pub day: NaiveDate,
    pub planned: bool,
    pub done: bool,
    pub minutes: i64,
    pub count: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ItemStats {
    pub id: String,
    pub title: String,
    pub block: BoundaryId,
    pub repeat: Repeat,
    pub weekdays: Option<Weekdays>,
    pub measure: Measure,
    pub target: Option<i64>,
    pub planned_days: usize,
    pub done_days: usize,
    pub minutes: i64,
    pub count: i64,
    pub series: Vec<DayStat>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PrayerStats {
    pub prayer: BoundaryId,
    pub marked_days: usize,
    pub days: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Stats {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub days: usize,
    pub items: Vec<ItemStats>,
    pub prayers: Vec<PrayerStats>,
    pub marked_total: usize,
    pub prayer_slots: usize,
    /// One row per day in the range, oldest first.
    pub daily: Vec<DayTotals>,
    /// The range as a whole.
    pub totals: Totals,
}

/// Everything that happened on one day, summed over its items.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DayTotals {
    pub day: NaiveDate,
    /// Items planned that day.
    pub planned: usize,
    /// Planned items that were done.
    pub done: usize,
    /// Minutes spent on all items.
    pub minutes: i64,
    /// Counts recorded on counter items.
    pub count: i64,
    /// Prayers marked as prayed (0–5).
    pub prayers: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Totals {
    pub planned: usize,
    pub done: usize,
    pub minutes: i64,
    pub count: i64,
    pub prayers: usize,
    /// Days with at least one item done or any time spent.
    pub active_days: usize,
}

/// Per-item and per-prayer totals over an inclusive range of days.
pub fn stats(
    store: &Store,
    config: &Config,
    now: DateTime<Utc>,
    first: NaiveDate,
    last: NaiveDate,
) -> Result<Stats, Error> {
    let reports = history(store, config, now, first, last)?;
    let days = reports.len();
    let mut items: Vec<ItemStats> = Vec::new();
    for item in store.items()?.into_iter().filter(Item::is_live) {
        let series: Vec<DayStat> = reports
            .iter()
            .map(|report| {
                let row = report.items.iter().find(|r| r.id == item.id);
                DayStat {
                    day: report.day,
                    planned: row.is_some(),
                    done: row.is_some_and(|r| r.state == State::Done),
                    minutes: row.map_or(0, |r| r.minutes),
                    count: row.map_or(0, |r| r.count),
                }
            })
            .collect();
        // Repeating items are always listed, even before their first
        // occurrence; one-time items only when planned in the range.
        if item.repeat == Repeat::None && !series.iter().any(|d| d.planned) {
            continue;
        }
        items.push(ItemStats {
            planned_days: series.iter().filter(|d| d.planned).count(),
            done_days: series.iter().filter(|d| d.done).count(),
            minutes: series.iter().map(|d| d.minutes).sum(),
            count: series.iter().map(|d| d.count).sum(),
            id: item.id,
            title: item.title,
            block: item.block,
            repeat: item.repeat,
            weekdays: item.weekdays,
            measure: item.measure,
            target: item.target,
            series,
        });
    }
    items.sort_by(|a, b| a.block.cmp(&b.block).then_with(|| a.title.cmp(&b.title)));
    let marks: Vec<PrayerMark> = store
        .marks_between(first, last)?
        .into_iter()
        .filter(PrayerMark::is_live)
        .collect();
    let prayers: Vec<PrayerStats> = BoundaryId::PRAYERS
        .into_iter()
        .map(|prayer| PrayerStats {
            prayer,
            marked_days: marks.iter().filter(|m| m.prayer == prayer).count(),
            days,
        })
        .collect();
    let daily: Vec<DayTotals> = reports
        .iter()
        .map(|report| DayTotals {
            day: report.day,
            planned: report.items.len(),
            done: report
                .items
                .iter()
                .filter(|r| r.state == State::Done)
                .count(),
            minutes: report.items.iter().map(|r| r.minutes).sum(),
            count: report.items.iter().map(|r| r.count).sum(),
            prayers: marks.iter().filter(|m| m.day == report.day).count(),
        })
        .collect();
    let totals = Totals {
        planned: daily.iter().map(|d| d.planned).sum(),
        done: daily.iter().map(|d| d.done).sum(),
        minutes: daily.iter().map(|d| d.minutes).sum(),
        count: daily.iter().map(|d| d.count).sum(),
        prayers: marks.len(),
        active_days: daily.iter().filter(|d| d.done > 0 || d.minutes > 0).count(),
    };
    Ok(Stats {
        from: first,
        to: last,
        days,
        daily,
        totals,
        marked_total: marks.len(),
        prayer_slots: days * BoundaryId::PRAYERS.len(),
        items,
        prayers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::NewItem;
    use crate::planner::test_support::{at, config, date, ids, titled};
    use crate::planner::{add, log, mark};

    #[test]
    fn stats_count_planned_and_done_days_and_prayers() {
        let mut store = Store::open_in_memory().unwrap();
        let read = NewItem {
            repeat: Some(Repeat::Weekly),
            weekdays: Some(Weekdays::WORKWEEK),
            measure: Some(Measure::Minutes),
            date: Some(date(16)),
            ..titled("Read")
        };
        let read = add(&mut store, &config(), at(20, 13, 0), ids(), read).unwrap();
        log(
            &mut store,
            &config(),
            at(20, 13, 0),
            ids(),
            &read.id,
            25,
            Some(date(17)),
        )
        .unwrap();
        mark(
            &mut store,
            &config(),
            at(20, 13, 0),
            BoundaryId::Asr,
            Some(date(17)),
        )
        .unwrap();
        // 2026-11-16 is a Monday: the range holds Mon–Sun.
        let report = stats(&store, &config(), at(20, 13, 0), date(16), date(22)).unwrap();
        assert_eq!(report.days, 7);
        let item = &report.items[0];
        assert_eq!((item.planned_days, item.minutes), (5, 25));
        assert_eq!(item.series.len(), 7);
        assert!(!item.series[5].planned, "Saturday is not planned");
        assert_eq!(report.prayers[2].marked_days, 1);
        assert_eq!((report.marked_total, report.prayer_slots), (1, 35));
        assert_eq!(report.daily.len(), 7);
        let tuesday = &report.daily[1];
        assert_eq!(
            (tuesday.planned, tuesday.minutes, tuesday.prayers),
            (1, 25, 1)
        );
        assert_eq!(report.daily[5].planned, 0);
        assert_eq!((report.totals.planned, report.totals.minutes), (5, 25));
        assert_eq!((report.totals.prayers, report.totals.active_days), (1, 1));
    }

    #[test]
    fn stats_list_routines_before_their_first_occurrence() {
        let mut store = Store::open_in_memory().unwrap();
        // 2026-11-22 is a Sunday; the routine starts on Monday.
        let gym = NewItem {
            repeat: Some(Repeat::Weekly),
            weekdays: Some(Weekdays::parse_list("mon,wed").unwrap()),
            ..titled("Gym")
        };
        add(&mut store, &config(), at(22, 13, 0), ids(), gym).unwrap();
        add(&mut store, &config(), at(22, 13, 0), ids(), titled("Once")).unwrap();
        let report = stats(&store, &config(), at(22, 13, 0), date(22), date(22)).unwrap();
        let titles: Vec<_> = report
            .items
            .iter()
            .map(|i| (i.title.as_str(), i.planned_days))
            .collect();
        assert!(titles.contains(&("Gym", 0)));
        assert!(titles.contains(&("Once", 1)));
        let earlier = stats(&store, &config(), at(22, 13, 0), date(20), date(21)).unwrap();
        assert!(earlier.items.iter().all(|i| i.title != "Once"));
    }
}
