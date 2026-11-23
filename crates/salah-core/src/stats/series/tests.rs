use super::*;
use crate::stats::measures::Measures;
use crate::stats::table::tests::{World, close, item, on_item, range, world};
use crate::test_support::date;

fn run(w: &World, interval: Interval, split: Option<Split>, ids: &[&str], f: Filter) -> Trend {
    let ids: Vec<String> = ids.iter().map(|s| (*s).to_owned()).collect();
    series(
        &w.store,
        w.calendar(),
        range(14, 25),
        f,
        interval,
        split,
        &ids,
    )
    .unwrap()
}

fn find<'a>(t: &'a Trend, key: &str, measure: &str) -> &'a Series {
    t.series
        .iter()
        .find(|s| s.key == key && s.measure == measure)
        .unwrap_or_else(|| panic!("no series {key}/{measure}"))
}

fn values(s: &Series) -> Vec<Option<f64>> {
    s.points.iter().map(|p| p.value).collect()
}

#[test]
fn without_a_split_each_measure_is_one_series_with_gaps_and_zero_counts() {
    let w = world();
    let t = run(
        &w,
        Interval::Day,
        None,
        &["minutes", "focus", "sessions"],
        Filter::default(),
    );
    assert_eq!(t.periods.len(), 12);
    assert_eq!(t.periods[0], date(14));
    let keys: Vec<&str> = t.series.iter().map(|s| s.measure.as_str()).collect();
    assert_eq!(keys, ["minutes", "focus", "sessions"]);
    let minutes = find(&t, ALL, "minutes");
    assert_eq!(
        (minutes.label.as_str(), minutes.unit.as_str()),
        ("Minutes", "min")
    );
    assert_eq!(minutes.color, None);
    // 14 Nov is index 0: 17 Nov index 3 holds 30 + a 0-minute tick, 18 Nov 10, 24 Nov 20.
    assert_eq!(minutes.points[0].value, Some(0.0));
    assert_eq!(minutes.points[3].value, Some(30.0));
    assert_eq!(minutes.points[4].value, Some(10.0));
    assert_eq!(minutes.points[10].value, Some(20.0));
    assert_eq!(find(&t, ALL, "sessions").points[3].value, Some(2.0));
    let focus = find(&t, ALL, "focus");
    assert_eq!(focus.scale_max, Some(5));
    assert_eq!(focus.points[0].value, None, "a day without data is a gap");
    assert_eq!(focus.points[0].sessions, 0);
    assert_eq!(focus.points[10].value, None, "no focus recorded that day");
    assert_eq!(focus.points[10].sessions, 1);
    // minute-weighted: (5 * 30 + 2 * 1) / 31 on the day with the tick.
    close(focus.points[3].value, 152.0 / 31.0);
}

#[test]
fn weeks_start_on_monday_and_clip_the_first_period() {
    let w = world();
    let t = run(
        &w,
        Interval::Week,
        None,
        &["minutes", "distractions", "distractions_per_hour"],
        Filter::default(),
    );
    assert_eq!(t.periods, [date(14), date(16), date(23)]);
    let minutes = values(find(&t, ALL, "minutes"));
    assert_eq!(minutes, [Some(0.0), Some(40.0), Some(20.0)]);
    let d = find(&t, ALL, "distractions");
    assert_eq!(values(d), [None, Some(4.0), None]);
    let rate = find(&t, ALL, "distractions_per_hour");
    assert_eq!(rate.unit, "per hour");
    close(rate.points[1].value, 4.0 * 60.0 / 40.0);
    assert_eq!(rate.points[2].value, None);
}

#[test]
fn split_by_item_ranks_by_minutes_and_keeps_unassigned_as_none() {
    let w = world();
    let t = run(
        &w,
        Interval::Week,
        Some(Split::Item),
        &["minutes"],
        Filter::default(),
    );
    let keys: Vec<&str> = t.series.iter().map(|s| s.key.as_str()).collect();
    // Read 30 min, Walk 20 min, the project-only Gym session ("none") 10 min.
    assert_eq!(keys, [w.read.as_str(), w.loose.as_str(), "none"]);
    assert_eq!(t.series[0].label, "Read");
    assert_eq!(t.series[0].color.as_deref(), Some(w.quran.color.as_str()));
    assert_eq!(t.series[2].label, "No task");
    assert_eq!(values(&t.series[0]), [Some(0.0), Some(30.0), Some(0.0)]);
    assert_eq!(t.series[1].color, None);
}

#[test]
fn split_by_project_and_tag_count_a_session_in_each_tag() {
    let w = world();
    let t = run(
        &w,
        Interval::Week,
        Some(Split::Project),
        &["minutes", "focus"],
        Filter::default(),
    );
    assert_eq!(t.series.len(), 6);
    assert_eq!(t.series[0].label, "Quran · Minutes");
    assert_eq!(t.series[1].label, "Quran · Focus");
    assert_eq!(find(&t, &w.gym.id, "minutes").points[1].value, Some(10.0));
    assert_eq!(find(&t, "none", "minutes").points[2].value, Some(20.0));
    let t = run(
        &w,
        Interval::Week,
        Some(Split::Tag),
        &["minutes"],
        Filter::default(),
    );
    // Study: Quran 30 + Gym 10; Deen: Quran 30; No tag: the walk.
    assert_eq!(find(&t, &w.study.id, "minutes").points[1].value, Some(40.0));
    assert_eq!(find(&t, &w.deen.id, "minutes").points[1].value, Some(30.0));
    assert_eq!(find(&t, "none", "minutes").points[2].value, Some(20.0));
    assert_eq!(find(&t, "none", "minutes").label, "No tag");
}

#[test]
fn more_than_eight_keys_merge_the_rest_into_other_once() {
    let mut w = world();
    let mut ids = Vec::new();
    for n in 0..9 {
        ids.push(item(&mut w.store, &format!("Task {n}"), None));
    }
    w.store
        .write(|tx| {
            for (n, id) in ids.iter().enumerate() {
                // Task 0 logs 20 minutes, Task 8 logs 12: the smallest three go to Other.
                crate::stats::table::tests::log(
                    tx,
                    on_item(id),
                    19,
                    Some(20 - n as i64),
                    &[("focus", 3.0)],
                );
            }
            Ok(())
        })
        .unwrap();
    let t = run(
        &w,
        Interval::Week,
        Some(Split::Item),
        &["minutes", "focus"],
        Filter::default(),
    );
    let keys: Vec<&str> = t
        .series
        .iter()
        .filter(|s| s.measure == "minutes")
        .map(|s| s.key.as_str())
        .collect();
    assert_eq!(keys.len(), 9);
    assert_eq!(*keys.last().unwrap(), OTHER);
    let other = find(&t, OTHER, "minutes");
    assert_eq!(other.label, "Other · Minutes");
    assert_eq!(other.color, None);
    // Read 30, Task 0..=6 and Walk 20, 19..14 ... ranks: 30, 20, 20, 19, 18, 17, 16, 15 | 14, 13, 12, 10.
    assert_eq!(other.points[1].value, Some(14.0 + 13.0 + 12.0 + 10.0));
    assert_eq!(other.points[1].sessions, 4);
    close(find(&t, OTHER, "focus").points[1].value, 127.0 / 49.0);
}

#[test]
fn filters_narrow_the_sessions_and_none_selects_unassigned_work() {
    let w = world();
    let f = Filter {
        project_id: Some("none".into()),
        ..Filter::default()
    };
    let t = run(&w, Interval::Week, None, &["minutes", "sessions"], f);
    assert_eq!(
        values(find(&t, ALL, "minutes")),
        [Some(0.0), Some(0.0), Some(20.0)]
    );
    let f = Filter {
        tag_id: Some(w.deen.id.clone()),
        ..Filter::default()
    };
    let t = run(&w, Interval::Week, None, &["sessions"], f);
    assert_eq!(
        values(find(&t, ALL, "sessions")),
        [Some(0.0), Some(2.0), Some(0.0)]
    );
    let f = Filter {
        measures: Measures {
            minutes_min: Some(15),
            ..Measures::default()
        },
        ..Filter::default()
    };
    let t = run(&w, Interval::Week, None, &["sessions"], f);
    assert_eq!(
        values(find(&t, ALL, "sessions")),
        [Some(0.0), Some(1.0), Some(1.0)]
    );
}

#[test]
fn bad_requests_are_rejected() {
    let w = world();
    let go = |r: Range, ids: &[&str]| {
        let ids: Vec<String> = ids.iter().map(|s| (*s).to_owned()).collect();
        series(
            &w.store,
            w.calendar(),
            r,
            Filter::default(),
            Interval::Day,
            None,
            &ids,
        )
        .unwrap_err()
        .code()
    };
    assert_eq!(go(range(14, 25), &[]), "validation");
    assert_eq!(go(range(14, 25), &["nope"]), "not_found");
    let long = Range {
        from: date(1),
        to: date(1) + chrono::Days::new(730),
    };
    assert_eq!(go(long, &["minutes"]), "validation");
}
