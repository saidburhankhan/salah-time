use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use salah::day::BoundaryId;
use salah::export::{self, Export, FORMAT, MAX_BYTES, VERSION};
use salah::item::{Entry, EntryKind, Item, Measure, Repeat};
use salah::schedule::BlockKey;
use salah::store::Store;

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
        measure: Measure::Minutes,
        target: Some(20),
        day: day(),
        created_at: time(1),
        updated_at,
        deleted_at: None,
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
    }
}

fn data(items: Vec<Item>, entries: Vec<Entry>) -> Export {
    Export {
        format: FORMAT.into(),
        version: VERSION,
        exported_at: time(10),
        items,
        entries,
    }
}

#[test]
fn round_trip_reproduces_every_row_including_tombstones() {
    let source = Store::open_in_memory().unwrap();
    let mut deleted_item = item("item-deleted", time(4));
    deleted_item.deleted_at = Some(time(5));
    let live_item = item("item-live", time(2));
    source.put_item(&deleted_item).unwrap();
    source.put_item(&live_item).unwrap();

    let mut deleted_entry = entry("entry-deleted", "item-deleted", time(4));
    deleted_entry.deleted_at = Some(time(5));
    source.put_entry(&deleted_entry).unwrap();
    source
        .put_entry(&entry("entry-live", "item-live", time(3)))
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
    store.put_item(&current).unwrap();

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
        store.put_item(&item("existing", time(2))).unwrap();
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
