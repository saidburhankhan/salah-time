use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use salah_core::export::{self, Export, FORMAT, MAX_BYTES, VERSION};
use salah_domain::day::BlockKey;
use salah_domain::day::BoundaryId;
use salah_domain::item::{Entry, EntryKind, Item, Measure, Repeat};
use salah_store::Store;

fn time(hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 20, hour, 0, 0).unwrap()
}

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()
}

fn item(id: &str, updated_at: DateTime<Utc>) -> Item {
    Item {
        id: id.into(),
        title: "Read Quran".into(),
        block: BoundaryId::Fajr,
        repeat: Repeat::Daily,
        weekdays: None,
        measure: Measure::Minutes,
        target: Some(20),
        position: 0,
        day: day(),
        created_at: time(1),
        updated_at,
        deleted_at: None,
        version: 1,
    }
}

fn entry(id: &str, item_id: &str, updated_at: DateTime<Utc>) -> Entry {
    Entry {
        id: id.into(),
        item_id: item_id.into(),
        block: BlockKey {
            day: day(),
            start: BoundaryId::Fajr,
        },
        kind: EntryKind::Timer,
        value: None,
        started_at: Some(time(2)),
        ended_at: Some(time(3)),
        created_at: time(1),
        updated_at,
        deleted_at: None,
        version: 1,
    }
}

fn data(items: Vec<Item>, entries: Vec<Entry>) -> Export {
    Export {
        format: FORMAT.into(),
        version: VERSION,
        exported_at: time(10),
        items,
        entries,
        prayer_marks: Vec::new(),
    }
}

#[test]
fn round_trip_reproduces_every_row_including_tombstones() {
    let mut source = Store::open_in_memory().unwrap();
    let mut deleted_item = item("item-deleted", time(4));
    deleted_item.deleted_at = Some(time(5));
    let live_item = item("item-live", time(2));
    let mut deleted_entry = entry("entry-deleted", "item-deleted", time(4));
    deleted_entry.deleted_at = Some(time(5));
    source
        .write(|tx| {
            tx.put_item(&deleted_item)?;
            tx.put_item(&live_item)?;
            tx.put_entry(&deleted_entry)?;
            tx.put_entry(&entry("entry-live", "item-live", time(3)))
        })
        .unwrap();

    let exported = export::export(&source, time(10)).unwrap();
    let json = serde_json::to_vec(&exported).unwrap();
    let parsed = export::parse(&json).unwrap();
    let mut destination = Store::open_in_memory().unwrap();
    let summary = export::import(&mut destination, &parsed).unwrap();

    assert_eq!(summary.added, 4);
    assert_eq!(destination.items().unwrap(), source.items().unwrap());
    assert_eq!(destination.entries().unwrap(), source.entries().unwrap());

    let second = export::import(&mut destination, &parsed).unwrap();
    assert_eq!(second.added, 0);
    assert_eq!(second.updated, 0);
    assert_eq!(second.unchanged, 4);
}

#[test]
fn newer_updated_at_wins_and_older_is_ignored() {
    let mut store = Store::open_in_memory().unwrap();
    let mut current = item("item", time(3));
    current.title = "Current".into();
    store.write(|tx| tx.put_item(&current)).unwrap();

    let mut newer = current.clone();
    newer.title = "Newer".into();
    newer.updated_at = time(4);
    let summary = export::import(&mut store, &data(vec![newer], vec![])).unwrap();
    assert_eq!(summary.updated, 1);
    assert_eq!(store.item("item").unwrap().unwrap().title, "Newer");

    let mut older = current;
    older.title = "Older".into();
    older.updated_at = time(2);
    let summary = export::import(&mut store, &data(vec![older], vec![])).unwrap();
    assert_eq!(summary.updated, 0);
    assert_eq!(summary.unchanged, 1);
    assert_eq!(store.item("item").unwrap().unwrap().title, "Newer");
}

#[test]
fn invalid_rows_and_headers_do_not_change_the_store() {
    let invalid_exports = [
        {
            let mut bad = item("new-item", time(2));
            bad.title = " not trimmed ".into();
            data(vec![bad], vec![])
        },
        data(
            vec![item("new-item", time(2))],
            vec![entry("entry", "missing", time(2))],
        ),
        {
            let mut bad = data(vec![item("new-item", time(2))], vec![]);
            bad.format = "other".into();
            bad
        },
        {
            let mut bad = data(vec![item("new-item", time(2))], vec![]);
            bad.version = VERSION + 1;
            bad
        },
    ];

    for invalid in invalid_exports {
        let mut store = Store::open_in_memory().unwrap();
        store
            .write(|tx| tx.put_item(&item("existing", time(2))))
            .unwrap();
        let before = store.items().unwrap();
        assert!(export::import(&mut store, &invalid).is_err());
        assert_eq!(store.items().unwrap(), before);
        assert!(store.entries().unwrap().is_empty());
    }
}

#[test]
fn parse_rejects_wrong_format_newer_version_and_oversize_input() {
    let mut wrong_format = data(vec![], vec![]);
    wrong_format.format = "other".into();
    assert!(export::parse(&serde_json::to_vec(&wrong_format).unwrap()).is_err());

    let mut newer = data(vec![], vec![]);
    newer.version = VERSION + 1;
    let error = export::parse(&serde_json::to_vec(&newer).unwrap()).unwrap_err();
    assert!(error.contains("newer than this salah"));

    assert!(export::parse(&vec![b' '; MAX_BYTES + 1]).is_err());
}

#[test]
fn version_one_exports_still_import() {
    // A v1 file has no prayer marks and no item position/weekdays.
    let v1 = r#"{"format":"salah-export","version":1,"exported_at":"2026-11-20T10:00:00Z",
        "items":[{"id":"old","title":"Old","block":"fajr","repeat":"daily","measure":"check",
                  "target":null,"day":"2026-11-20","created_at":"2026-11-20T01:00:00Z",
                  "updated_at":"2026-11-20T01:00:00Z","deleted_at":null}],
        "entries":[]}"#;
    let data = export::parse(v1.as_bytes()).unwrap();
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(export::import(&mut store, &data).unwrap().added, 1);
    let old = store.item("old").unwrap().unwrap();
    assert_eq!((old.position, old.weekdays), (0, None));
}

#[test]
fn version_three_exports_entity_versions_and_import_preserves_them() {
    use salah_domain::item::PrayerMark;

    let mut source = Store::open_in_memory().unwrap();
    let mut row = item("versioned", time(2));
    source.write(|tx| tx.put_item(&row)).unwrap();
    row.title = "Updated".into();
    row.updated_at = time(3);
    source.write(|tx| tx.put_item(&row)).unwrap();
    let mark = PrayerMark {
        id: PrayerMark::id_for(day(), BoundaryId::Asr),
        day: day(),
        prayer: BoundaryId::Asr,
        marked_at: time(5),
        created_at: time(5),
        updated_at: time(5),
        deleted_at: None,
        version: 1,
    };
    source
        .write(|tx| {
            tx.put_entry(&entry("versioned-entry", "versioned", time(3)))?;
            tx.put_mark(&mark)
        })
        .unwrap();
    assert_eq!(source.item("versioned").unwrap().unwrap().version, 2);

    let exported = export::export(&source, time(10)).unwrap();
    assert_eq!(exported.version, VERSION);
    assert_eq!(exported.items[0].version, 2);
    let value = serde_json::to_value(&exported).unwrap();
    assert_eq!(value["version"], VERSION);
    assert_eq!(value["items"][0]["version"], 2);
    assert_eq!(value["entries"][0]["version"], 1);
    assert_eq!(value["prayer_marks"][0]["version"], 1);
    assert!(value.get("changes").is_none());
    assert!(value.get("applied_ops").is_none());

    let mut destination = Store::open_in_memory().unwrap();
    export::import(&mut destination, &exported).unwrap();
    assert_eq!(destination.item("versioned").unwrap().unwrap().version, 2);
    assert_eq!(destination.mark(&mark.id).unwrap().unwrap().version, 1);
}

#[test]
fn version_two_exports_still_import_with_version_one_rows() {
    let v2 = r#"{"format":"salah-export","version":2,
        "exported_at":"2026-11-20T10:00:00Z",
        "items":[{"id":"old-v2","title":"Old v2","block":"fajr","repeat":"daily",
                  "measure":"check","target":null,"position":0,"weekdays":null,
                  "day":"2026-11-20","created_at":"2026-11-20T01:00:00Z",
                  "updated_at":"2026-11-20T01:00:00Z","deleted_at":null}],
        "entries":[],
        "prayer_marks":[{"id":"2026-11-20:asr","day":"2026-11-20","prayer":"asr",
                         "marked_at":"2026-11-20T02:00:00Z",
                         "created_at":"2026-11-20T02:00:00Z",
                         "updated_at":"2026-11-20T02:00:00Z","deleted_at":null}]}"#;
    let data = export::parse(v2.as_bytes()).unwrap();
    assert_eq!(data.items[0].version, 1);
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(export::import(&mut store, &data).unwrap().added, 2);
    assert_eq!(store.item("old-v2").unwrap().unwrap().version, 1);
    assert_eq!(store.mark("2026-11-20:asr").unwrap().unwrap().version, 1);
}

#[test]
fn prayer_marks_round_trip_and_reject_bad_ids() {
    use salah_domain::item::PrayerMark;
    let mark = PrayerMark {
        id: PrayerMark::id_for(day(), BoundaryId::Asr),
        day: day(),
        prayer: BoundaryId::Asr,
        marked_at: time(5),
        created_at: time(5),
        updated_at: time(5),
        deleted_at: None,
        version: 1,
    };
    let mut good = data(Vec::new(), Vec::new());
    good.prayer_marks = vec![mark.clone()];
    let bytes = serde_json::to_vec(&good).unwrap();
    let mut store = Store::open_in_memory().unwrap();
    export::import(&mut store, &export::parse(&bytes).unwrap()).unwrap();
    assert_eq!(store.marks().unwrap(), [mark]);

    let mut bad = good.clone();
    bad.prayer_marks[0].id = "2026-11-20:fajr".into();
    assert!(export::validate(&bad).is_err());
}
