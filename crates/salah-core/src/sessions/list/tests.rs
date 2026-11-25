use super::*;
use crate::WriteTx;
use crate::projects::{self, ProjectChanges};
use crate::sessions::{self, NewLog, SessionChanges, Target, Values};
use crate::test_support::{at, config, date, ids, project};
use crate::{items, tags};
use salah_domain::day::BoundaryId;
use salah_domain::tracking::{Project, Session};

fn range() -> Range {
    Range {
        from: date(14),
        to: date(25),
    }
}

/// Logged at `day` 13:`minute`, so creation order is the listing order.
fn log(
    tx: &WriteTx<'_>,
    target: Target,
    day: u32,
    minute: u32,
    minutes: Option<i64>,
    values: &[(&str, f64)],
    note: Option<&str>,
) -> Session {
    sessions::log(
        tx,
        &config(),
        at(day, 13, minute),
        ids(),
        NewLog {
            target,
            date: Some(date(day)),
            block: Some(BoundaryId::Dhuhr),
            minutes,
            values: values
                .iter()
                .map(|(id, v)| ((*id).to_owned(), *v))
                .collect::<Values>(),
            note: note.map(str::to_owned),
        },
    )
    .unwrap()
}

fn on_project(project: &Project) -> Target {
    Target {
        item_id: None,
        project_id: Some(project.id.clone()),
        epic_id: None,
    }
}

fn on_item(id: &str) -> Target {
    Target {
        item_id: Some(id.into()),
        project_id: None,
        epic_id: None,
    }
}

struct World {
    store: Store,
    quran: Project,
    gym: Project,
    read: String,
    walk: String,
    tag: String,
    /// Newest first: walk (24), gym timer (20), gym tick (19), read (18), quiet (17).
    walk_log: String,
    timer: String,
    tick: String,
    read_log: String,
    quiet: String,
}

fn world() -> World {
    let mut store = Store::open_in_memory().unwrap();
    let quran = project(&mut store, "Quran");
    let gym = project(&mut store, "Gym");
    let (read, walk, tag) = store
        .write(|tx| {
            let new = |title: &str, project_id: Option<String>| items::NewItem {
                title: title.into(),
                project_id,
                date: Some(date(16)),
                ..Default::default()
            };
            let tag = tags::add(tx, at(16, 6, 0), ids(), "Deen", "#556677")?;
            let changes = ProjectChanges {
                tag_ids: Some(vec![tag.id.clone()]),
                ..Default::default()
            };
            projects::edit(
                tx,
                &crate::test_support::config(),
                at(16, 6, 0),
                &quran.id,
                changes,
                None,
            )?;
            let read = items::add(
                tx,
                &config(),
                at(16, 6, 0),
                ids(),
                new("Read", Some(quran.id.clone())),
            )?;
            let walk = items::add(tx, &config(), at(16, 6, 0), ids(), new("Walk", None))?;
            Ok((read.id, walk.id, tag.id))
        })
        .unwrap();
    let mut world = store
        .write(|tx| {
            let quiet = log(
                tx,
                on_project(&quran),
                17,
                0,
                Some(15),
                &[("focus", 3.0)],
                None,
            );
            let read_log = log(
                tx,
                on_item(&read),
                18,
                0,
                Some(30),
                &[("focus", 1.0), ("quality", 4.0)],
                Some("Surah Al-Mulk, ÄYAT 1–10"),
            );
            let tick = log(
                tx,
                on_project(&gym),
                19,
                0,
                None,
                &[("focus", 2.0)],
                Some("tick note"),
            );
            let walk_log = log(tx, on_item(&walk), 24, 0, Some(45), &[], Some("Long walk"));
            Ok((quiet.id, read_log.id, tick.id, walk_log.id))
        })
        .map(|(quiet, read_log, tick, walk_log)| World {
            store,
            quran: quran.clone(),
            gym: gym.clone(),
            read,
            walk,
            tag,
            walk_log,
            timer: String::new(),
            tick,
            read_log,
            quiet,
        })
        .unwrap();
    // A timer on day 20 with only a timestamped entry, stopped after 20 minutes.
    let timer = world
        .store
        .write(|tx| {
            let started = sessions::start(tx, &config(), at(20, 13, 0), ids(), on_project(&gym))?;
            sessions::append_note(tx, &config(), at(20, 13, 5), ids(), None, "Ünïcode entry")?;
            sessions::stop(
                tx,
                &config(),
                at(20, 13, 20),
                ids(),
                None,
                Default::default(),
            )?;
            Ok(started.id)
        })
        .unwrap();
    world.timer = timer;
    world
}

fn run(w: &World, filter: Filter, query: SessionQuery) -> SessionList {
    list(&w.store, range(), filter, query).unwrap()
}

fn ids_of(page: &SessionList) -> Vec<&str> {
    page.rows.iter().map(|r| r.session.id.as_str()).collect()
}

fn all() -> SessionQuery {
    SessionQuery {
        with_note: false,
        ..SessionQuery::default()
    }
}

#[test]
fn rows_are_finished_sessions_newest_first_with_labels_and_entries() {
    let w = world();
    let page = run(&w, Filter::default(), all());
    assert_eq!(
        ids_of(&page),
        [&w.walk_log, &w.timer, &w.tick, &w.read_log, &w.quiet].map(String::as_str)
    );
    assert_eq!(page.next, None);
    let read = &page.rows[3];
    assert_eq!(read.item.as_ref().unwrap().title, "Read");
    assert_eq!(read.project.as_ref().unwrap().name, "Quran");
    assert_eq!(read.session.values.get("quality"), Some(&4.0));
    assert_eq!(read.session.stale, Some(false));
    let timer = &page.rows[1];
    assert_eq!(timer.session.minutes, Some(20));
    assert_eq!(timer.session.notes.len(), 1);
    assert_eq!(timer.session.notes[0].text, "Ünïcode entry");
    let walk = &page.rows[0];
    assert!(walk.project.is_none(), "an unassigned task has no project");
    let tick = &page.rows[2];
    assert_eq!((tick.session.minutes, tick.item.as_ref()), (None, None));
}

#[test]
fn with_note_defaults_on_and_counts_timestamped_entries() {
    let w = world();
    let page = run(&w, Filter::default(), SessionQuery::default());
    assert_eq!(
        ids_of(&page),
        [&w.walk_log, &w.timer, &w.tick, &w.read_log].map(String::as_str),
        "the quiet session has neither a note nor an entry"
    );
    let serde: SessionQuery = serde_json::from_str("{}").unwrap();
    assert!(serde.with_note, "the protocol default is true");
}

#[test]
fn q_matches_the_note_or_any_entry_case_insensitively() {
    let w = world();
    let q = |text: &str| {
        let query = SessionQuery {
            q: Some(text.into()),
            with_note: false,
            ..SessionQuery::default()
        };
        run(&w, Filter::default(), query)
    };
    assert_eq!(ids_of(&q("al-mulk")), [w.read_log.as_str()]);
    assert_eq!(ids_of(&q("äyat")), [w.read_log.as_str()], "Unicode folds");
    assert_eq!(ids_of(&q("ÜNÏCODE")), [w.timer.as_str()], "entries match");
    assert!(q("nothing like this").rows.is_empty());
    assert_eq!(q("100%_").rows.len(), 0, "no LIKE wildcards");
    assert_eq!(q("   ").rows.len(), 5, "blank text is no filter");
    // A removed entry no longer matches.
    let mut w = w;
    let entry = run(&w, Filter::default(), all()).rows[1].session.notes[0]
        .id
        .clone();
    w.store
        .write(|tx| {
            let changes = SessionChanges {
                remove_note_ids: vec![entry.clone()],
                ..Default::default()
            };
            sessions::edit(tx, &config(), at(25, 9, 0), &w.timer, changes, None)
        })
        .unwrap();
    let query = SessionQuery {
        q: Some("unicode".into()),
        ..SessionQuery::default()
    };
    assert!(run(&w, Filter::default(), query).rows.is_empty());
    assert!(
        !ids_of(&run(&w, Filter::default(), SessionQuery::default())).contains(&w.timer.as_str())
    );
}

#[test]
fn duration_and_metric_ranges_are_inclusive_and_need_the_value() {
    let w = world();
    let minutes = |min: Option<i64>, max: Option<i64>| {
        let query = SessionQuery {
            minutes_min: min,
            minutes_max: max,
            ..all()
        };
        run(&w, Filter::default(), query)
    };
    assert_eq!(
        ids_of(&minutes(Some(20), Some(30))),
        [w.timer.as_str(), w.read_log.as_str()]
    );
    assert_eq!(ids_of(&minutes(Some(45), None)), [w.walk_log.as_str()]);
    assert_eq!(
        minutes(None, Some(1000)).rows.len(),
        4,
        "a tick has no duration"
    );
    let values = |bounds: &[(&str, Option<f64>, Option<f64>)]| {
        let query = SessionQuery {
            values: bounds
                .iter()
                .map(|(id, min, max)| {
                    (
                        (*id).to_owned(),
                        ValueRange {
                            min: *min,
                            max: *max,
                        },
                    )
                })
                .collect(),
            ..all()
        };
        run(&w, Filter::default(), query)
    };
    assert_eq!(
        ids_of(&values(&[("focus", Some(1.0), Some(2.0))])),
        [w.tick.as_str(), w.read_log.as_str()]
    );
    assert_eq!(
        ids_of(&values(&[("focus", Some(3.0), None)])),
        [w.quiet.as_str()]
    );
    assert_eq!(
        ids_of(&values(&[
            ("focus", None, Some(2.0)),
            ("quality", Some(4.0), None)
        ])),
        [w.read_log.as_str()],
        "every metric range must match"
    );
    assert!(
        values(&[("distractions", Some(0.0), None)]).rows.is_empty(),
        "no value, no match"
    );
}

#[test]
fn tag_project_and_item_filters_accept_none() {
    let w = world();
    let filtered = |tag: Option<&str>, project: Option<&str>, item: Option<&str>| {
        let filter = Filter {
            tag_id: tag.map(str::to_owned),
            project_id: project.map(str::to_owned),
            item_id: item.map(str::to_owned),
            ..Filter::default()
        };
        run(&w, filter, all())
    };
    assert_eq!(
        ids_of(&filtered(None, Some("none"), None)),
        [w.walk_log.as_str()]
    );
    assert_eq!(
        ids_of(&filtered(None, Some(&w.gym.id), None)),
        [w.timer.as_str(), w.tick.as_str()]
    );
    assert_eq!(
        ids_of(&filtered(Some(&w.tag), None, None)),
        [w.read_log.as_str(), w.quiet.as_str()]
    );
    assert_eq!(
        ids_of(&filtered(Some("none"), None, None)),
        [&w.walk_log, &w.timer, &w.tick].map(String::as_str)
    );
    assert_eq!(
        ids_of(&filtered(None, None, Some(&w.read))),
        [w.read_log.as_str()]
    );
    assert_eq!(
        ids_of(&filtered(None, None, Some(&w.walk))),
        [w.walk_log.as_str()]
    );
    assert_eq!(
        ids_of(&filtered(None, Some(&w.quran.id), Some("none"))),
        [w.quiet.as_str()]
    );
    let error = list(
        &w.store,
        range(),
        Filter {
            project_id: Some("missing".into()),
            ..Filter::default()
        },
        all(),
    )
    .unwrap_err();
    assert_eq!(error.code(), "not_found");
}

#[test]
fn the_cursor_pages_without_gaps_or_repeats() {
    let w = world();
    let mut seen = Vec::new();
    let mut before = None;
    loop {
        let query = SessionQuery {
            limit: Some(2),
            before: before.clone(),
            ..all()
        };
        let page = run(&w, Filter::default(), query);
        seen.extend(ids_of(&page).into_iter().map(str::to_owned));
        match page.next {
            Some(next) => {
                assert_eq!(page.rows.len(), 2);
                assert!(next.ends_with(&format!("|{}", page.rows[1].session.id)));
                before = Some(next);
            }
            None => break,
        }
    }
    let everything: Vec<String> = ids_of(&run(&w, Filter::default(), all()))
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(seen, everything);
    let exact = SessionQuery {
        limit: Some(5),
        ..all()
    };
    assert_eq!(
        run(&w, Filter::default(), exact).next,
        None,
        "a full last page"
    );
}

#[test]
fn invalid_queries_are_validation_errors() {
    let w = world();
    let fails = |query: SessionQuery, field: &str| {
        let error = list(&w.store, range(), Filter::default(), query).unwrap_err();
        assert_eq!(error.code(), "validation", "{field}");
        assert!(format!("{error:?}").contains(field), "{error:?}");
    };
    fails(
        SessionQuery {
            limit: Some(0),
            ..all()
        },
        "limit",
    );
    fails(
        SessionQuery {
            limit: Some(501),
            ..all()
        },
        "limit",
    );
    fails(
        SessionQuery {
            minutes_min: Some(-1),
            ..all()
        },
        "minutes_min",
    );
    fails(
        SessionQuery {
            minutes_min: Some(10),
            minutes_max: Some(5),
            ..all()
        },
        "minutes_max",
    );
    fails(
        SessionQuery {
            values: [(
                "focus".to_owned(),
                ValueRange {
                    min: Some(4.0),
                    max: Some(2.0),
                },
            )]
            .into(),
            ..all()
        },
        "values",
    );
    for cursor in [
        "nonsense",
        "2026-11-20|2026-11-20T08:20:00.000Z|",
        "2026-11-20T08:20:00.000Z|abc",
        "yesterday|2026-11-20T08:20:00.000Z|abc",
    ] {
        fails(
            SessionQuery {
                before: Some(cursor.into()),
                ..all()
            },
            "before",
        );
    }
    let unknown = SessionQuery {
        values: [("nope".to_owned(), ValueRange::default())].into(),
        ..all()
    };
    let error = list(&w.store, range(), Filter::default(), unknown).unwrap_err();
    assert_eq!(error.code(), "not_found");
    let reversed = Range {
        from: date(20),
        to: date(14),
    };
    assert_eq!(
        list(&w.store, reversed, Filter::default(), all())
            .unwrap_err()
            .code(),
        "validation"
    );
    let full = SessionQuery {
        limit: Some(500),
        ..all()
    };
    assert_eq!(run(&w, Filter::default(), full).rows.len(), 5);
}

#[test]
fn removed_and_running_sessions_are_not_listed() {
    let mut w = world();
    w.store
        .write(|tx| {
            sessions::remove(tx, &config(), at(25, 9, 0), &w.read_log, None)?;
            sessions::start(tx, &config(), at(25, 13, 0), ids(), on_project(&w.quran))?;
            sessions::append_note(tx, &config(), at(25, 13, 1), ids(), None, "running note")
        })
        .unwrap();
    let page = run(&w, Filter::default(), all());
    assert_eq!(
        ids_of(&page),
        [&w.walk_log, &w.timer, &w.tick, &w.quiet].map(String::as_str)
    );
}

#[test]
fn a_session_logged_later_sorts_under_its_own_day() {
    let mut w = world();
    let late = w
        .store
        .write(|tx| {
            Ok(sessions::log(
                tx,
                &config(),
                at(25, 20, 0),
                ids(),
                NewLog {
                    target: on_project(&w.gym),
                    date: Some(date(15)),
                    block: Some(BoundaryId::Asr),
                    minutes: Some(10),
                    ..NewLog::default()
                },
            )?
            .id)
        })
        .unwrap();
    let page = run(&w, Filter::default(), all());
    assert_eq!(ids_of(&page).last(), Some(&late.as_str()));
    let first = SessionQuery {
        limit: Some(5),
        ..all()
    };
    let next = run(&w, Filter::default(), first).next.unwrap();
    assert!(next.starts_with("2026-11-17|"), "{next}");
    let rest = SessionQuery {
        before: Some(next),
        ..all()
    };
    assert_eq!(ids_of(&run(&w, Filter::default(), rest)), [late.as_str()]);
}
