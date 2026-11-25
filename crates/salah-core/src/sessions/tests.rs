use super::*;
fn stop(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    id: Option<&str>,
    values: Values,
    note: Option<String>,
) -> Result<Session, Error> {
    Ok(super::stop(
        tx,
        config,
        now,
        ids(),
        id,
        Stop {
            values,
            note,
            ..Stop::default()
        },
    )?
    .stopped)
}
use crate::test_support::{at, config, date, ids, project};

fn rated(focus: f64, quality: f64) -> Values {
    Values::from([("focus".into(), focus), ("quality".into(), quality)])
}

fn on_project(id: &str) -> Target {
    Target {
        item_id: None,
        project_id: Some(id.into()),
        epic_id: None,
    }
}

#[test]
fn a_timer_stops_with_rounded_minutes_and_optional_values() {
    let mut store = Store::open_in_memory().unwrap();
    let quran = project(&mut store, "Quran");
    let started = store
        .write(|tx| start(tx, &config(), at(20, 13, 0), ids(), on_project(&quran.id)))
        .unwrap();
    assert!(started.is_running());
    assert_eq!(started.block.start, BoundaryId::Dhuhr);
    let second =
        store.write(|tx| start(tx, &config(), at(20, 13, 1), ids(), on_project(&quran.id)));
    assert_eq!(second.unwrap_err().code(), "conflict");

    let invalid =
        store.write(|tx| stop(tx, &config(), at(20, 13, 25), None, rated(6.0, 3.0), None));
    assert_eq!(invalid.unwrap_err().code(), "validation");

    let stopped = store
        .write(|tx| {
            stop(
                tx,
                &config(),
                at(20, 13, 25),
                Some(&started.id),
                Values::from([("focus".into(), 4.0)]),
                Some(" good ".into()),
            )
        })
        .unwrap();
    assert_eq!(stopped.minutes, Some(25));
    assert_eq!(stopped.note.as_deref(), Some("good"));
    assert_eq!(running(&store).unwrap(), None);
}

#[test]
fn a_log_without_minutes_is_a_tick_and_copies_the_item_project() {
    let mut store = Store::open_in_memory().unwrap();
    let quran = project(&mut store, "Quran");
    let item = store
        .write(|tx| {
            items::add(
                tx,
                &config(),
                at(20, 6, 0),
                ids(),
                items::NewItem {
                    title: "Read".into(),
                    block: Some(BoundaryId::Fajr),
                    project_id: Some(quran.id.clone()),
                    ..items::NewItem::default()
                },
            )
        })
        .unwrap();
    let tick = store
        .write(|tx| {
            log(
                tx,
                &config(),
                at(20, 13, 0),
                ids(),
                NewLog {
                    target: Target {
                        item_id: Some(item.id.clone()),
                        project_id: None,
                        epic_id: None,
                    },
                    date: Some(date(19)),
                    values: rated(5.0, 5.0),
                    ..NewLog::default()
                },
            )
        })
        .unwrap();
    assert_eq!(tick.minutes, None);
    assert_eq!(tick.project_id.as_deref(), Some(quran.id.as_str()));
    assert_eq!(
        tick.block,
        BlockKey {
            day: date(19),
            start: BoundaryId::Fajr
        },
        "a dated log lands in the item's own block"
    );
    let without_target =
        store.write(|tx| log(tx, &config(), at(20, 13, 0), ids(), NewLog::default()));
    assert_eq!(without_target.unwrap_err().code(), "validation");
}

#[test]
fn edit_and_remove_respect_versions() {
    let mut store = Store::open_in_memory().unwrap();
    let quran = project(&mut store, "Quran");
    let logged = store
        .write(|tx| {
            log(
                tx,
                &config(),
                at(20, 13, 0),
                ids(),
                NewLog {
                    target: on_project(&quran.id),
                    minutes: Some(10),
                    values: rated(3.0, 3.0),
                    ..NewLog::default()
                },
            )
        })
        .unwrap();
    let changes = SessionChanges {
        minutes: Some(40),
        block: Some(BoundaryId::Asr),
        ..SessionChanges::default()
    };
    let edited = store
        .write(|tx| {
            edit(
                tx,
                &config(),
                at(20, 14, 0),
                &logged.id,
                changes.clone(),
                Some(1),
            )
        })
        .unwrap();
    assert_eq!(
        (edited.minutes, edited.block.start),
        (Some(40), BoundaryId::Asr)
    );
    let stale = store.write(|tx| remove(tx, &config(), at(20, 14, 0), &logged.id, Some(1)));
    assert_eq!(stale.unwrap_err().code(), "conflict");
    store
        .write(|tx| remove(tx, &config(), at(20, 14, 0), &logged.id, Some(2)))
        .unwrap();
    assert_eq!(find(&store, &logged.id).unwrap_err().code(), "not_found");
}

#[test]
fn prompt_lists_the_project_metrics_in_order() {
    let mut store = Store::open_in_memory().unwrap();
    let quran = project(&mut store, "Quran");
    let prompt = prompt(&store, Some(&quran.id)).unwrap();
    let ids: Vec<_> = prompt
        .iter()
        .map(|m| (m.metric_id.as_str(), m.required))
        .collect();
    assert_eq!(
        ids,
        [
            ("focus", false),
            ("quality", false),
            ("distractions", false)
        ]
    );
    assert_eq!(prompt[0].scale_max, Some(5));
}

#[test]
fn a_forgotten_timer_ends_at_its_day_sleep_boundary() {
    let mut store = Store::open_in_memory().unwrap();
    let quran = project(&mut store, "Quran");
    store
        .write(|tx| start(tx, &config(), at(20, 13, 0), ids(), on_project(&quran.id)))
        .unwrap();
    let invalid = store.write(|tx| {
        super::stop(
            tx,
            &config(),
            at(23, 9, 0),
            ids(),
            None,
            Stop {
                minutes: Some(601),
                ..Stop::default()
            },
        )
        .map(|result| result.stopped)
    });
    assert!(matches!(
        invalid,
        Err(Error::Validation {
            field: "minutes",
            ..
        })
    ));
    assert!(running(&store).unwrap().is_some());
    let stopped = store
        .write(|tx| stop(tx, &config(), at(23, 9, 0), None, rated(3.0, 3.0), None))
        .unwrap();
    // 13:00 to the 23:00 sleep boundary of the session's day.
    assert_eq!(stopped.minutes, Some(600));
    assert_eq!(stopped.ended_at, Some(at(20, 23, 0)));

    // The frozen windows still govern this timer after bedtime is changed.
    let changed = Config {
        sleep_time: "14:00".into(),
        ..config()
    };
    let past = day::day_plan(&mut store, &changed, at(23, 9, 0), Some(date(20))).unwrap();
    let report = crate::stats::table::table(
        &store,
        crate::stats::Calendar {
            config: &changed,
            today: date(23),
        },
        crate::stats::Range {
            from: date(20),
            to: date(23),
        },
        crate::stats::Filter::default(),
        crate::stats::table::Group::Block,
    )
    .unwrap();
    assert_eq!(report.totals.sessions, 1);
    assert_eq!(report.totals.minutes, 600);
    assert_eq!(
        report
            .rows
            .iter()
            .map(|row| row.key.as_str())
            .collect::<Vec<_>>(),
        ["dhuhr", "asr", "maghrib", "isha"]
    );
    for row in &report.rows {
        let block = past
            .blocks
            .iter()
            .find(|b| b.start.as_str() == row.key)
            .unwrap();
        assert_eq!(row.minutes, block.unplanned_minutes);
        if block.start != BoundaryId::Dhuhr {
            assert_eq!(row.minutes, block.capacity_minutes);
        }
    }
    assert_eq!(report.rows.iter().map(|row| row.minutes).sum::<i64>(), 600);
}

#[test]
fn early_sleep_ends_the_timer_and_the_planning_day_even_before_isha() {
    let mut store = Store::open_in_memory().unwrap();
    let project = project(&mut store, "Book");
    let config = Config {
        sleep_time: "14:00".into(),
        ..config()
    };
    store
        .write(|tx| start(tx, &config, at(20, 13, 0), ids(), on_project(&project.id)))
        .unwrap();
    let after = at(20, 21, 0);
    assert_eq!(day::current(&config, after).unwrap(), date(21));
    let stopped = store
        .write(|tx| stop(tx, &config, after, None, rated(3.0, 3.0), None))
        .unwrap();
    assert_eq!(stopped.ended_at, Some(at(20, 14, 0)));
    assert_eq!(stopped.minutes, Some(60));
    assert!(store.day_written(date(20)).unwrap());
    let day = day::day_plan(&mut store, &config, after, Some(date(20))).unwrap();
    assert_eq!(day.sleep.time, "14:00");
    let dhuhr = day
        .blocks
        .iter()
        .find(|b| b.start == BoundaryId::Dhuhr)
        .unwrap();
    let start = day
        .prayers
        .iter()
        .find(|p| p.id == BoundaryId::Dhuhr)
        .unwrap();
    assert_eq!(
        dhuhr.capacity_minutes,
        14 * 60 - salah_domain::day::parse_clock_minutes(&start.time).unwrap()
    );
    assert!(
        day.blocks
            .iter()
            .filter(|b| b.start > BoundaryId::Dhuhr)
            .all(|b| b.capacity_minutes == 0)
    );
}

#[test]
fn archived_metrics_reject_new_samples_and_preserve_existing_samples() {
    let mut store = Store::open_in_memory().unwrap();
    let pages = store
        .write(|tx| {
            metrics::add(
                tx,
                at(20, 9, 0),
                ids(),
                metrics::NewMetric {
                    name: "Pages".into(),
                    unit: "pages".into(),
                    kind: MetricKind::Count,
                    scale_max: None,
                    rollup: None,
                },
            )
        })
        .unwrap();
    let book = store
        .write(|tx| {
            projects::add(
                tx,
                at(20, 9, 0),
                ids(),
                projects::NewProject {
                    name: "Book".into(),
                    metrics: Some(vec![projects::MetricChoice {
                        metric_id: pages.id.clone(),
                        required: true,
                    }]),
                    ..projects::NewProject::default()
                },
            )
        })
        .unwrap();
    let logged = store
        .write(|tx| {
            log(
                tx,
                &config(),
                at(20, 13, 0),
                ids(),
                NewLog {
                    target: on_project(&book.id),
                    minutes: Some(10),
                    values: Values::from([(pages.id.clone(), 12.0)]),
                    ..NewLog::default()
                },
            )
        })
        .unwrap();
    let archive = metrics::MetricChanges {
        archived: Some(true),
        ..metrics::MetricChanges::default()
    };
    store
        .write(|tx| metrics::edit(tx, at(20, 14, 0), &pages.id, archive.clone(), None))
        .unwrap();

    assert!(prompt(&store, Some(&book.id)).unwrap().is_empty());
    store
        .write(|tx| start(tx, &config(), at(20, 14, 0), ids(), on_project(&book.id)))
        .unwrap();
    let archived_sample = Values::from([(pages.id.clone(), 1.0)]);
    let invalid = store
        .write(|tx| {
            stop(
                tx,
                &config(),
                at(20, 14, 30),
                None,
                archived_sample.clone(),
                None,
            )
        })
        .unwrap_err();
    assert_eq!(invalid.code(), "validation");
    assert!(running(&store).unwrap().is_some());
    store
        .write(|tx| stop(tx, &config(), at(20, 14, 30), None, Values::new(), None))
        .expect("a stop without values succeeds");

    let keep = SessionChanges {
        values: Some(Values::from([(pages.id.clone(), 12.0)])),
        note: Some(Some("kept".into())),
        ..SessionChanges::default()
    };
    store
        .write(|tx| edit(tx, &config(), at(20, 15, 0), &logged.id, keep.clone(), None))
        .expect("an existing archived value stays valid");
    let visible_only = SessionChanges {
        values: Some(Values::new()),
        ..Default::default()
    };
    let edited = store
        .write(|tx| {
            edit(
                tx,
                &config(),
                at(20, 15, 0),
                &logged.id,
                visible_only.clone(),
                None,
            )
        })
        .unwrap();
    assert_eq!(edited.value(&pages.id), Some(12.0));
    let change = SessionChanges {
        values: Some(Values::from([(pages.id.clone(), 13.0)])),
        ..SessionChanges::default()
    };
    let rejected = store.write(|tx| {
        edit(
            tx,
            &config(),
            at(20, 15, 0),
            &logged.id,
            change.clone(),
            None,
        )
    });
    assert_eq!(rejected.unwrap_err().code(), "validation");
}

#[test]
fn a_running_view_caps_max_minutes_at_its_day_sleep_boundary() {
    let mut store = Store::open_in_memory().unwrap();
    let quran = project(&mut store, "Quran");
    let started = store
        .write(|tx| start(tx, &config(), at(20, 21, 0), ids(), on_project(&quran.id)))
        .unwrap();
    let view = |now| SessionView::at(&store, &config(), now, &started).unwrap();
    assert_eq!(
        view(at(20, 21, 0)).max_minutes,
        Some(1),
        "stop accepts at least 1"
    );
    assert_eq!(view(at(20, 21, 40)).max_minutes, Some(40));
    // Overnight: the 23:00 Sleep boundary cuts it to 120, not 600.
    let overnight = view(at(21, 7, 0));
    assert_eq!(
        (overnight.max_minutes, overnight.stale),
        (Some(120), Some(true))
    );
    let json = serde_json::to_value(&overnight).unwrap();
    assert_eq!(json["max_minutes"], 120);
    let too_many = store.write(|tx| {
        super::stop(
            tx,
            &config(),
            at(21, 7, 0),
            ids(),
            None,
            Stop {
                minutes: Some(121),
                ..Stop::default()
            },
        )
    });
    assert_eq!(too_many.err().map(|e| e.code()), Some("validation"));
    let stopped = store
        .write(|tx| stop(tx, &config(), at(21, 7, 0), None, Values::new(), None))
        .unwrap();
    let finished = SessionView::at(&store, &config(), at(21, 7, 0), &stopped).unwrap();
    assert_eq!(finished.max_minutes, None);
    assert!(
        serde_json::to_value(&finished)
            .unwrap()
            .get("max_minutes")
            .is_none()
    );
}

#[test]
fn a_timer_past_sleep_refuses_counts_and_notes_until_stopped() {
    let mut store = Store::open_in_memory().unwrap();
    let quran = project(&mut store, "Quran");
    store
        .write(|tx| start(tx, &config(), at(20, 21, 0), ids(), on_project(&quran.id)))
        .unwrap();
    store
        .write(|tx| super::bump(tx, &config(), at(20, 22, 0), None, "distractions", 1))
        .unwrap();
    store
        .write(|tx| {
            super::append_note(
                tx,
                &config(),
                at(20, 22, 59),
                ids(),
                super::NewNote {
                    text: "late",
                    ..Default::default()
                },
            )
        })
        .unwrap();
    let bumped =
        store.write(|tx| super::bump(tx, &config(), at(21, 8, 0), None, "distractions", 1));
    let noted = store.write(|tx| {
        super::append_note(
            tx,
            &config(),
            at(21, 8, 0),
            ids(),
            super::NewNote {
                text: "morning",
                ..Default::default()
            },
        )
    });
    for result in [bumped, noted] {
        match result {
            Err(Error::Validation { field, message }) => {
                assert_eq!(field, "id");
                assert!(message.contains("past Sleep"), "{message}");
            }
            other => panic!("expected a validation error, got {other:?}"),
        }
    }
    let timer = running(&store).unwrap().unwrap();
    assert_eq!(timer.value("distractions"), Some(1.0));
    assert_eq!(store.session_notes(&timer.id).unwrap().len(), 1);
    let stopped = store
        .write(|tx| stop(tx, &config(), at(21, 8, 0), None, Values::new(), None))
        .unwrap();
    assert_eq!(stopped.ended_at, Some(at(20, 23, 0)));
}
