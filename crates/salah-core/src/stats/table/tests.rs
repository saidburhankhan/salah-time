use super::*;
use crate::metrics::{self, MetricChanges, NewMetric};
use crate::projects::{self, ProjectChanges};
use crate::sessions::{self, NewLog, Target, Values};
use crate::test_support::{at, config, date, ids, project};
use crate::{WriteTx, items, tags};
use salah_domain::tracking::{Project, Tag};

/// 2026-11-16 and 2026-11-23 are Mondays.
pub(crate) fn range(from: u32, to: u32) -> Range {
    Range {
        from: date(from),
        to: date(to),
    }
}

pub(crate) fn log(
    tx: &WriteTx<'_>,
    target: Target,
    day: u32,
    minutes: Option<i64>,
    values: &[(&str, f64)],
) -> Session {
    sessions::log(
        tx,
        &config(),
        at(25, 13, 0),
        ids(),
        NewLog {
            target,
            date: Some(date(day)),
            block: Some(salah_domain::day::BoundaryId::Dhuhr),
            minutes,
            values: values
                .iter()
                .map(|(id, v)| ((*id).to_owned(), *v))
                .collect::<Values>(),
            ..NewLog::default()
        },
    )
    .unwrap()
}

pub(crate) fn on_project(project: &Project) -> Target {
    Target {
        item_id: None,
        project_id: Some(project.id.clone()),
        epic_id: None,
    }
}

pub(crate) fn on_item(id: &str) -> Target {
    Target {
        item_id: Some(id.into()),
        project_id: None,
        epic_id: None,
    }
}

pub(crate) fn item(store: &mut Store, title: &str, project_id: Option<&str>) -> String {
    store
        .write(|tx| {
            items::add(
                tx,
                &config(),
                at(16, 6, 0),
                ids(),
                items::NewItem {
                    title: title.into(),
                    project_id: project_id.map(str::to_owned),
                    planned_minutes: Some(20),
                    date: Some(date(16)),
                    ..Default::default()
                },
            )
        })
        .unwrap()
        .id
}

pub(crate) fn tag(store: &mut Store, name: &str, projects: &[&Project]) -> Tag {
    store
        .write(|tx| {
            let tag = tags::add(tx, at(16, 9, 0), ids(), name, "#445566")?;
            for project in projects {
                let mut tag_ids = tx.project(&project.id)?.unwrap().tag_ids;
                tag_ids.push(tag.id.clone());
                projects::edit(
                    tx,
                    at(16, 9, 0),
                    &project.id,
                    ProjectChanges {
                        tag_ids: Some(tag_ids),
                        ..Default::default()
                    },
                    None,
                )?;
            }
            Ok(tag)
        })
        .unwrap()
}

/// Quran (tags Deen, Study), Gym (Study), and a task without a project.
pub(crate) struct World {
    config: crate::config::Config,
    pub(crate) store: Store,
    pub(crate) quran: Project,
    pub(crate) gym: Project,
    pub(crate) read: String,
    pub(crate) loose: String,
    pub(crate) deen: Tag,
    pub(crate) study: Tag,
}

impl World {
    pub(crate) fn calendar(&self) -> Calendar<'_> {
        Calendar {
            config: &self.config,
            today: date(25),
        }
    }
}

pub(crate) fn world() -> World {
    let mut store = Store::open_in_memory().unwrap();
    let quran = project(&mut store, "Quran");
    let gym = project(&mut store, "Gym");
    let deen = tag(&mut store, "Deen", &[&quran]);
    let study = tag(&mut store, "Study", &[&quran, &gym]);
    let read = item(&mut store, "Read", Some(&quran.id));
    let loose = item(&mut store, "Walk", None);
    store
        .write(|tx| {
            // Week 16–22: Quran 30 min focus 5, a Read tick focus 2, Gym 10 min focus 1.
            log(
                tx,
                on_item(&read),
                17,
                Some(30),
                &[("focus", 5.0), ("distractions", 3.0)],
            );
            log(tx, on_item(&read), 17, None, &[("focus", 2.0)]);
            log(
                tx,
                on_project(&gym),
                18,
                Some(10),
                &[("focus", 1.0), ("distractions", 1.0)],
            );
            // Week 23–29: an unassigned walk of 20 minutes.
            log(tx, on_item(&loose), 24, Some(20), &[("quality", 4.0)]);
            Ok(())
        })
        .unwrap();
    World {
        config: config(),
        store,
        quran,
        gym,
        read,
        loose,
        deen,
        study,
    }
}

pub(crate) fn build(w: &World, group: Group, filter: Filter) -> Table {
    table(&w.store, w.calendar(), range(14, 25), filter, group).unwrap()
}

pub(crate) fn value(values: &BTreeMap<String, Option<f64>>, id: &str) -> Option<f64> {
    *values.get(id).unwrap_or_else(|| panic!("no {id} cell"))
}

pub(crate) fn row<'a>(table: &'a Table, key: &str) -> &'a Row {
    table
        .rows
        .iter()
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no row {key}"))
}

pub(crate) fn close(actual: Option<f64>, expected: f64) {
    let actual = actual.expect("a value");
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[test]
fn weeks_start_on_monday_are_clipped_to_the_range_and_keep_empty_weeks() {
    let w = world();
    let t = build(&w, Group::default(), Filter::default());
    assert_eq!(t.group, Group::Week);
    let keys: Vec<&str> = t.rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["2026-W46", "2026-W47", "2026-W48"]);
    let first = &t.rows[0];
    assert_eq!((first.from, first.to), (Some(date(14)), Some(date(15))));
    assert_eq!(first.label, "14–15 Nov 2026");
    assert_eq!((first.minutes, first.sessions), (0, 0));
    assert!(
        first.values.values().all(Option::is_none),
        "empty week: nulls"
    );
    let busy = &t.rows[1];
    assert_eq!((busy.from, busy.to), (Some(date(16)), Some(date(22))));
    assert_eq!((busy.minutes, busy.sessions), (40, 3));
    // (5·30 + 2·1 + 1·10) / 41: minutes weigh an avg, a tick weighs 1.
    close(value(&busy.values, "focus"), 162.0 / 41.0);
    close(value(&busy.values, "distractions"), 4.0);
    assert_eq!(value(&busy.values, "quality"), None);
    let last = &t.rows[2];
    assert_eq!((last.from, last.to), (Some(date(23)), Some(date(25))));
    assert_eq!((last.minutes, last.sessions), (20, 1));
    assert_eq!((t.totals.minutes, t.totals.sessions), (60, 4));
    close(value(&t.totals.values, "focus"), 162.0 / 41.0);
    close(value(&t.totals.values, "quality"), 4.0);
}

#[test]
fn week_labels_span_months_and_years() {
    let label = |a: (i32, u32, u32), b: (i32, u32, u32)| {
        let d = |(y, m, d)| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        span_label(d(a), d(b))
    };
    assert_eq!(label((2026, 11, 30), (2026, 12, 6)), "30 Nov – 6 Dec 2026");
    assert_eq!(
        label((2026, 12, 28), (2027, 1, 3)),
        "28 Dec 2026 – 3 Jan 2027"
    );
    assert_eq!(label((2026, 12, 28), (2026, 12, 28)), "28 Dec 2026");
    let span = Range {
        from: NaiveDate::from_ymd_opt(2026, 12, 30).unwrap(),
        to: NaiveDate::from_ymd_opt(2027, 1, 5).unwrap(),
    };
    let keys: Vec<String> = weeks(&span).into_iter().map(|w| w.0).collect();
    assert_eq!(keys, ["2026-W53", "2027-W01"], "ISO week years");
}

#[test]
fn day_rows_cover_every_day_in_the_range() {
    let w = world();
    let t = build(&w, Group::Day, Filter::default());
    assert_eq!(t.rows.len(), 12);
    assert_eq!(t.rows[0].key, "2026-11-14");
    assert_eq!(t.rows[0].label, "Sat 14 Nov 2026");
    let busy = row(&t, "2026-11-17");
    assert_eq!((busy.minutes, busy.sessions), (30, 2));
    assert_eq!((busy.from, busy.to), (Some(date(17)), Some(date(17))));
    assert_eq!(row(&t, "2026-11-19").sessions, 0);
    let sum: i64 = t.rows.iter().map(|r| r.minutes).sum();
    assert_eq!(sum, t.totals.minutes);
}

#[test]
fn project_rows_include_no_project_and_omit_projects_without_sessions() {
    let mut w = world();
    project(&mut w.store, "Idle");
    let t = build(&w, Group::Project, Filter::default());
    let keys: Vec<&str> = t.rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, [w.quran.id.as_str(), "none", w.gym.id.as_str()]);
    let quran = row(&t, &w.quran.id);
    assert_eq!(
        (quran.label.as_str(), quran.color.as_deref()),
        ("Quran", Some(w.quran.color.as_str()))
    );
    assert_eq!((quran.minutes, quran.sessions, quran.from), (30, 2, None));
    let none = row(&t, "none");
    assert_eq!(
        (none.label.as_str(), none.color.as_deref()),
        ("No project", None)
    );
    assert_eq!(none.minutes, 20);
    let sum: i64 = t.rows.iter().map(|r| r.sessions).sum();
    assert_eq!(sum, t.totals.sessions, "rows cover every session");
}

#[test]
fn tag_rows_count_a_session_in_each_tag_but_totals_count_it_once() {
    let w = world();
    let t = build(&w, Group::Tag, Filter::default());
    let study = row(&t, &w.study.id);
    assert_eq!((study.minutes, study.sessions), (40, 3));
    let deen = row(&t, &w.deen.id);
    assert_eq!(
        (deen.minutes, deen.sessions, deen.label.as_str()),
        (30, 2, "Deen")
    );
    let untagged = row(&t, "none");
    assert_eq!((untagged.label.as_str(), untagged.minutes), ("No tag", 20));
    assert_eq!(t.rows[0].key, w.study.id, "most minutes first");
    assert_eq!((t.totals.minutes, t.totals.sessions), (60, 4));
}

#[test]
fn item_rows_label_tasks_and_group_project_only_work_as_no_task() {
    let mut w = world();
    w.store
        .write(|tx| {
            log(tx, on_project(&w.quran), 19, Some(5), &[]);
            items::remove(tx, &config(), at(25, 14, 0), &w.read, None)?;
            Ok(())
        })
        .unwrap();
    let t = build(&w, Group::Item, Filter::default());
    let read = row(&t, &w.read);
    assert_eq!(
        (read.label.as_str(), read.minutes, read.sessions),
        ("Read", 30, 2)
    );
    assert_eq!(read.color.as_deref(), Some(w.quran.color.as_str()));
    let walk = row(&t, &w.loose);
    assert_eq!((walk.label.as_str(), walk.color.as_deref()), ("Walk", None));
    let none = row(&t, "none");
    assert_eq!(
        (none.label.as_str(), none.minutes, none.sessions),
        ("No task", 15, 2)
    );
}

#[test]
fn filters_intersect_and_none_selects_unassigned_work() {
    let w = world();
    let filter = |tag: Option<&str>, project: Option<&str>, item: Option<&str>| Filter {
        tag_id: tag.map(str::to_owned),
        project_id: project.map(str::to_owned),
        item_id: item.map(str::to_owned),
        ..Filter::default()
    };
    let totals = |f: Filter| {
        let t = build(&w, Group::Project, f);
        (t.totals.minutes, t.totals.sessions)
    };
    assert_eq!(totals(filter(None, Some("none"), None)), (20, 1));
    assert_eq!(totals(filter(Some("none"), None, None)), (20, 1));
    assert_eq!(totals(filter(None, None, Some("none"))), (10, 1));
    assert_eq!(totals(filter(Some(&w.study.id), None, None)), (40, 3));
    assert_eq!(
        totals(filter(Some(&w.deen.id), Some(&w.gym.id), None)),
        (0, 0)
    );
    assert_eq!(
        totals(filter(Some(&w.study.id), Some(&w.quran.id), Some(&w.read))),
        (30, 2)
    );
    assert_eq!(totals(filter(None, Some("none"), Some(&w.read))), (0, 0));
    let echoed = build(&w, Group::Week, filter(None, Some("none"), None));
    assert_eq!(echoed.filter.project_id.as_deref(), Some("none"));
    assert_eq!(echoed.rows.len(), 3, "empty weeks stay under a filter");
    for missing in [
        filter(Some("missing"), None, None),
        filter(None, Some("missing"), None),
        filter(None, None, Some("missing")),
    ] {
        let error = table(&w.store, w.calendar(), range(14, 25), missing, Group::Week).unwrap_err();
        assert_eq!(error.code(), "not_found");
    }
}

#[test]
fn ranges_are_checked_and_running_or_removed_sessions_do_not_count() {
    let mut w = world();
    w.store
        .write(|tx| {
            let gone = log(tx, on_project(&w.gym), 20, Some(50), &[]);
            sessions::remove(tx, &config(), at(25, 13, 5), &gone.id, None)?;
            sessions::start(tx, &config(), at(25, 13, 10), ids(), on_project(&w.gym))?;
            Ok(())
        })
        .unwrap();
    let t = table(
        &w.store,
        w.calendar(),
        range(14, 25),
        Filter::default(),
        Group::Project,
    )
    .unwrap();
    assert_eq!((t.totals.minutes, t.totals.sessions), (60, 4));
    let reversed = table(
        &w.store,
        w.calendar(),
        range(20, 14),
        Filter::default(),
        Group::Week,
    );
    assert_eq!(reversed.unwrap_err().code(), "validation");
    let long = Range {
        from: date(1),
        to: date(1) + Days::new(730),
    };
    let error = table(&w.store, w.calendar(), long, Filter::default(), Group::Day).unwrap_err();
    assert_eq!(error.code(), "validation");
    let widest = Range {
        from: date(1),
        to: date(1) + Days::new(729),
    };
    let t = table(
        &w.store,
        w.calendar(),
        widest,
        Filter::default(),
        Group::Week,
    )
    .unwrap();
    assert_eq!(t.totals.sessions, 4);
}

#[test]
fn columns_lead_with_minutes_and_sessions_and_archived_metrics_need_data() {
    let mut w = world();
    let (pages, mood) = w
        .store
        .write(|tx| {
            let count = |name: &str| NewMetric {
                name: name.into(),
                unit: "x".into(),
                kind: MetricKind::Count,
                scale_max: None,
                rollup: None,
            };
            let pages = metrics::add(tx, at(16, 9, 0), ids(), count("Pages"))?;
            let mood = metrics::add(tx, at(16, 9, 0), ids(), count("Mood"))?;
            let unused = metrics::add(tx, at(16, 9, 0), ids(), count("Unused"))?;
            log(tx, on_project(&w.gym), 18, Some(5), &[(&mood.id, 2.0)]);
            for id in [&mood.id, &unused.id] {
                let archive = MetricChanges {
                    archived: Some(true),
                    ..Default::default()
                };
                metrics::edit(tx, at(25, 9, 0), id, archive, None)?;
            }
            Ok((pages, mood))
        })
        .unwrap();
    let t = build(&w, Group::Week, Filter::default());
    let ids: Vec<&str> = t.columns.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "minutes",
            "sessions",
            "focus",
            "quality",
            "distractions",
            "distractions_per_hour",
            mood.id.as_str(),
            pages.id.as_str(),
        ],
        "an archived metric without data is omitted; an active one stays"
    );
    let per_hour = &t.columns[5];
    assert_eq!(per_hour.derived_from.as_deref(), Some("distractions"));
    assert_eq!(
        (per_hour.rollup, per_hour.unit.as_str()),
        (Rollup::Avg, "per hour")
    );
    assert!(t.columns[6].archived);
    assert_eq!(t.columns[2].scale_max, Some(5));
    assert_eq!(value(&t.totals.values, &pages.id), None);
    close(value(&t.totals.values, &mood.id), 2.0);
    assert_eq!(
        t.totals.values.len(),
        t.columns.len() - 2,
        "a cell per metric column"
    );
}

#[test]
fn a_project_filter_orders_metric_columns_like_its_prompt() {
    let mut w = world();
    w.store
        .write(|tx| {
            let metrics = vec![
                projects::MetricChoice {
                    metric_id: "distractions".into(),
                    required: false,
                },
                projects::MetricChoice {
                    metric_id: "focus".into(),
                    required: false,
                },
            ];
            let changes = ProjectChanges {
                metrics: Some(metrics),
                ..Default::default()
            };
            projects::edit(tx, at(25, 9, 0), &w.gym.id, changes, None)
        })
        .unwrap();
    let filter = Filter {
        project_id: Some(w.gym.id.clone()),
        ..Filter::default()
    };
    let t = build(&w, Group::Week, filter);
    let ids: Vec<&str> = t.columns.iter().skip(2).map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        ["distractions", "distractions_per_hour", "focus", "quality"]
    );
}

#[test]
fn distractions_per_hour_divides_the_sum_by_the_rows_hours() {
    let w = world();
    let t = build(&w, Group::Project, Filter::default());
    // Quran: 3 distractions in 30 minutes; Gym: 1 in 10 minutes.
    close(
        value(&row(&t, &w.quran.id).values, "distractions_per_hour"),
        6.0,
    );
    close(
        value(&row(&t, &w.gym.id).values, "distractions_per_hour"),
        6.0,
    );
    assert_eq!(
        value(&row(&t, "none").values, "distractions_per_hour"),
        None
    );
    // 4 distractions over the 60 minutes of every session in scope.
    close(value(&t.totals.values, "distractions_per_hour"), 4.0);
    let mut ticks = world();
    ticks
        .store
        .write(|tx| {
            log(
                tx,
                on_item(&ticks.loose),
                15,
                None,
                &[("distractions", 2.0)],
            );
            Ok(())
        })
        .unwrap();
    let t = build(&ticks, Group::Day, Filter::default());
    let only_tick = row(&t, "2026-11-15");
    close(value(&only_tick.values, "distractions"), 2.0);
    assert_eq!(
        value(&only_tick.values, "distractions_per_hour"),
        None,
        "no hours"
    );
}
