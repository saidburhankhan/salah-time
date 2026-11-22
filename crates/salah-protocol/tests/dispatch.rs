//! Dispatcher tests: every method has a happy path and an invalid-params
//! error, plus the handshake, replay, and version-conflict invariants.

use chrono::{TimeZone, Utc};
use salah_core::Store;
use salah_core::config::Config;
use salah_domain::error::Error;
use salah_domain::runtime::{Clock, FixedClock, IdGenerator, SequenceIds};
use salah_protocol::{
    ConfigRepository, Context, Dispatcher, INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND,
    PARSE_ERROR, Request, Response, RpcError,
};
use serde_json::{Value, json};
use std::sync::Mutex;

struct MemoryConfig {
    config: Mutex<Config>,
}

impl ConfigRepository for MemoryConfig {
    fn load(&self) -> Result<Config, Error> {
        Ok(self.config.lock().unwrap().clone())
    }

    fn save(&self, config: &Config) -> Result<(), Error> {
        *self.config.lock().unwrap() = config.clone();
        Ok(())
    }
}

struct Fixture {
    store: Store,
    config: MemoryConfig,
    clock: FixedClock,
    ids: SequenceIds,
    dispatcher: Dispatcher,
}

impl Fixture {
    fn new() -> Self {
        let now = Utc.with_ymd_and_hms(2031, 5, 14, 4, 30, 0).unwrap();
        Self {
            store: Store::open_in_memory().unwrap(),
            config: MemoryConfig {
                config: Mutex::new(Config {
                    method: "mwl".into(),
                    ..Config::default()
                }),
            },
            clock: FixedClock::new(now),
            ids: SequenceIds::new(),
            dispatcher: Dispatcher::new(),
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Response {
        let request = Request::new(json!(1), method, params);
        let Self {
            store,
            config,
            clock,
            ids,
            dispatcher,
        } = self;
        let mut context = Context {
            store,
            config,
            clock,
            ids,
        };
        dispatcher.dispatch(&request, &mut context)
    }

    fn line(&mut self, line: &str) -> Option<Response> {
        let Self {
            store,
            config,
            clock,
            ids,
            dispatcher,
        } = self;
        let mut context = Context {
            store,
            config,
            clock,
            ids,
        };
        dispatcher.handle_line(line, &mut context)
    }

    fn ok(&mut self, method: &str, params: Value) -> Value {
        let response = self.call(method, params);
        assert!(
            response.error.is_none(),
            "{method} failed: {:?}",
            response.error
        );
        response.result.unwrap()
    }

    fn err(&mut self, method: &str, params: Value) -> RpcError {
        let response = self.call(method, params);
        assert!(response.result.is_none(), "{method} unexpectedly succeeded");
        response.error.unwrap()
    }

    fn handshake(&mut self) -> Value {
        self.ok(
            "hello",
            json!({"protocol":[1],"client":"test/1","actor":"test"}),
        )
    }

    fn now(&self) -> chrono::DateTime<Utc> {
        self.clock.now()
    }
}

fn item_id(result: &Value) -> &str {
    result["changed"]["items"][0]["id"].as_str().unwrap()
}

fn item_version(result: &Value) -> i64 {
    result["changed"]["items"][0]["version"].as_i64().unwrap()
}

#[test]
fn handshake_is_required_and_negotiates_protocol_one() {
    let mut fixture = Fixture::new();
    let error = fixture.err("status.get", Value::Null);
    assert_eq!(error.code, INVALID_REQUEST);

    let error = fixture.err("hello", json!({"protocol":[2],"client":"old"}));
    assert_eq!(error.code, INVALID_PARAMS);

    let hello = fixture.handshake();
    assert_eq!(hello["protocol"], 1);
    assert_eq!(hello["server"], "salah 0.1.0");
    assert_eq!(hello["revision"], 0);
    assert_eq!(
        hello["capabilities"],
        json!(["timer.v1", "events.v1", "clock.v1"])
    );

    let error = fixture.err("not.a.method", json!({}));
    assert_eq!(error.code, METHOD_NOT_FOUND);

    let parsed = fixture.line("{not json").unwrap();
    assert_eq!(parsed.error.unwrap().code, PARSE_ERROR);
    let invalid = fixture.line("\"just a string\"").unwrap();
    assert_eq!(invalid.error.unwrap().code, INVALID_REQUEST);
}

#[test]
fn every_method_rejects_invalid_params_with_a_validation_kind() {
    let mut fixture = Fixture::new();
    fixture.handshake();
    for method in [
        "hello",
        "status.get",
        "day.get",
        "items.list",
        "stats.get",
        "history.get",
        "item.parse",
        "item.preview",
        "item.add",
        "item.edit",
        "item.move",
        "item.remove",
        "entry.done",
        "entry.undo",
        "entry.log",
        "timer.start",
        "timer.stop",
        "prayer.mark",
        "prayer.unmark",
        "config.get",
        "config.set",
        "export",
        "import",
        "subscribe",
    ] {
        let error = fixture.err(method, json!([]));
        assert_eq!(error.code, INVALID_PARAMS, "{method}: {error:?}");
        let data = error.data.unwrap_or_else(|| panic!("{method} has no data"));
        assert_eq!(data.kind, "validation", "{method}");
    }
}

#[test]
fn reads_and_parse_return_canonical_projections() {
    let mut fixture = Fixture::new();
    fixture.handshake();

    let status = fixture.ok("status.get", Value::Null);
    assert_eq!(status["day"], "2031-05-14");
    assert!(status["planner"]["items"].is_array());

    let day = fixture.ok("day.get", json!({"date":"2031-05-14"}));
    assert_eq!(day["day"], "2031-05-14");
    assert_eq!(day["prayers"].as_array().unwrap().len(), 5);

    let items = fixture.ok("items.list", Value::Null);
    assert_eq!(items, json!([]));

    let stats = fixture.ok("stats.get", json!({"days":7}));
    assert_eq!(stats["days"], 7);
    let history = fixture.ok("history.get", json!({}));
    assert_eq!(history.as_array().unwrap().len(), 7);

    let parsed = fixture.ok("item.parse", json!({"text":"read 20m after isha daily"}));
    assert_eq!(parsed["block"], "isha");
    assert_eq!(parsed["measure"], "minutes");
    assert_eq!(parsed["target"], 20);

    let preview = fixture.ok(
        "item.preview",
        json!({
            "block":"asr","repeat":"weekly","weekdays":["mon","wed","fri"],
            "measure":"minutes","target":20,"date":"2031-05-14"
        }),
    );
    assert_eq!(preview["schedule_text"], "Mon, Wed, Fri");
    assert_eq!(preview["next_occurrence"], "2031-05-14");
    assert!(preview["occurrences_this_month"].as_u64().unwrap() > 0);

    let config = fixture.ok("config.get", Value::Null);
    assert_eq!(config["method"], "mwl");

    // Read-side domain errors use the same stable kinds.
    let error = fixture.err("day.get", json!({"date":"not-a-date"}));
    assert_eq!(error.code, INVALID_PARAMS);
    let error = fixture.err("stats.get", json!({"from":"2031-05-01"}));
    assert_eq!(error.data.unwrap().kind, "validation");

    assert!(fixture.now().timestamp() > 0);
    assert!(!fixture.ids.new_id().is_empty());
}

#[test]
fn item_lifecycle_returns_changed_rows_versions_and_idempotent_replays() {
    let mut fixture = Fixture::new();
    fixture.handshake();

    let add = fixture.ok(
        "item.add",
        json!({
            "op_id":"op-add",
            "title":"Read Quran",
            "block":"fajr",
            "repeat":"daily",
            "measure":"minutes",
            "target":30
        }),
    );
    let id = item_id(&add).to_string();
    let version = item_version(&add);
    let revision = add["revision"].as_i64().unwrap();
    assert_eq!((version, revision), (1, 1));
    assert_eq!(add["changed"]["items"][0]["state"], "open");

    // A replay with the same op_id (even a different body) returns the stored
    // result and creates no revision.
    let replay = fixture.ok(
        "item.add",
        json!({
            "op_id":"op-add",
            "title":"Different body",
            "block":"asr"
        }),
    );
    assert_eq!(replay, add);

    let edit = fixture.ok(
        "item.edit",
        json!({
            "op_id":"op-edit",
            "id":id,
            "expected_version":version,
            "title":"Read more"
        }),
    );
    assert_eq!(item_version(&edit), 2);
    assert_eq!(edit["revision"], 2);
    assert_eq!(edit["changed"]["items"][0]["title"], "Read more");

    let conflict = fixture.err(
        "item.edit",
        json!({
            "op_id":"op-stale",
            "id":id,
            "expected_version":1,
            "title":"Stale"
        }),
    );
    assert_eq!(conflict.data.unwrap().kind, "conflict");

    let second = fixture.ok(
        "item.add",
        json!({"op_id":"op-add-2","title":"Second","block":"fajr"}),
    );
    let second_id = item_id(&second).to_string();
    let third = fixture.ok(
        "item.add",
        json!({"op_id":"op-add-3","title":"Third","block":"fajr"}),
    );
    let third_id = item_id(&third).to_string();

    let moved = fixture.ok(
        "item.move",
        json!({
            "op_id":"op-move",
            "id":second_id,
            "block":"fajr",
            "before_id":third_id
        }),
    );
    assert!(
        moved["changed"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == second_id)
    );

    let removed = fixture.ok("item.remove", json!({"op_id":"op-remove","id":third_id}));
    assert_eq!(removed["changed"]["deleted"], json!([third_id]));
    let error = fixture.err("item.remove", json!({"op_id":"op-missing","id":"nope"}));
    assert_eq!(error.data.unwrap().kind, "not_found");

    // Full ids only over RPC, even though the human CLI accepts prefixes.
    let prefix = &id[..8];
    let error = fixture.err(
        "item.edit",
        json!({"op_id":"op-prefix","id":prefix,"title":"Prefix"}),
    );
    assert_eq!(error.data.unwrap().kind, "not_found");

    let error = fixture.err("item.add", json!({"op_id":"op-empty"}));
    assert_eq!(error.data.unwrap().kind, "validation");
    let error = fixture.err(
        "item.move",
        json!({"op_id":"op-both","id":id,"block":"asr","after_id":id,"before_id":id}),
    );
    assert_eq!(error.data.unwrap().kind, "validation");
}

#[test]
fn entry_timer_and_prayer_mutations_report_affected_rows() {
    let mut fixture = Fixture::new();
    fixture.handshake();

    let add = fixture.ok(
        "item.add",
        json!({
            "op_id":"op-add",
            "title":"Study",
            "block":"fajr",
            "measure":"minutes",
            "target":30
        }),
    );
    let id = item_id(&add).to_string();

    let count = fixture.ok(
        "item.add",
        json!({"op_id":"op-count","title":"Pushups","measure":"count"}),
    );
    let count_id = item_id(&count).to_string();

    let done = fixture.ok(
        "entry.done",
        json!({"op_id":"op-done","item_id":id,"date":"2031-05-14"}),
    );
    assert_eq!(done["changed"]["items"][0]["state"], "done");
    assert_eq!(done["changed"]["entries"][0]["kind"], "done");

    let undone = fixture.ok(
        "entry.undo",
        json!({"op_id":"op-undo","item_id":id,"date":"2031-05-14"}),
    );
    assert_eq!(undone["undone"], 1);
    assert_eq!(undone["changed"]["items"][0]["state"], "open");

    let logged = fixture.ok(
        "entry.log",
        json!({"op_id":"op-log","item_id":id,"kind":"minutes","value":5}),
    );
    assert_eq!(logged["changed"]["items"][0]["minutes"], 5);
    assert_eq!(logged["changed"]["entries"][0]["value"], 5);

    let counted = fixture.ok(
        "entry.log",
        json!({"op_id":"op-count-log","item_id":count_id,"kind":"count","value":3}),
    );
    assert_eq!(counted["changed"]["items"][0]["count"], 3);

    let started = fixture.ok("timer.start", json!({"op_id":"op-start","item_id":id}));
    assert!(started["changed"]["items"][0]["timer_started_at"].is_string());
    let stopped = fixture.ok("timer.stop", json!({"op_id":"op-stop","item_id":id}));
    assert_eq!(stopped["changed"]["entries"][0]["kind"], "timer");
    assert!(stopped["changed"]["items"][0]["timer_started_at"].is_null());

    let marked = fixture.ok(
        "prayer.mark",
        json!({"op_id":"op-mark","prayer":"dhuhr","date":"2031-05-14"}),
    );
    assert_eq!(marked["changed"]["marks"][0]["prayer"], "dhuhr");
    assert_eq!(marked["changed"]["marks"][0]["version"], 1);
    let unmarked = fixture.ok(
        "prayer.unmark",
        json!({"op_id":"op-unmark","prayer":"dhuhr","date":"2031-05-14"}),
    );
    assert_eq!(unmarked["changed"]["marks"][0]["prayer"], "dhuhr");

    let error = fixture.err("entry.done", json!({"op_id":"op-missing","item_id":"nope"}));
    assert_eq!(error.data.unwrap().kind, "not_found");
    let error = fixture.err(
        "entry.log",
        json!({"op_id":"op-zero","item_id":id,"value":0}),
    );
    assert_eq!(error.data.unwrap().kind, "validation");
    // `kind` is authoritative: a count can be recorded for any item.
    let forced_count = fixture.ok(
        "entry.log",
        json!({"op_id":"op-kind","item_id":id,"kind":"count","value":1}),
    );
    assert_eq!(forced_count["changed"]["items"][0]["count"], 1);
    let error = fixture.err(
        "timer.start",
        json!({"op_id":"op-timer-missing","item_id":"nope"}),
    );
    assert_eq!(error.data.unwrap().kind, "not_found");
    let error = fixture.err("prayer.mark", json!({"op_id":"op-sleep","prayer":"sleep"}));
    assert_eq!(error.data.unwrap().kind, "validation");
}

#[test]
fn config_export_and_import_round_trip() {
    let mut fixture = Fixture::new();
    fixture.handshake();

    let updated = fixture.ok(
        "config.set",
        json!({"op_id":"op-config","sleep_time":"22:30"}),
    );
    assert_eq!(updated["sleep_time"], "22:30");
    // Replaying the same op id returns the stored result, even with a
    // different retry body, and does not overwrite the saved configuration.
    let replay = fixture.ok(
        "config.set",
        json!({"op_id":"op-config","sleep_time":"21:00"}),
    );
    assert_eq!(replay, updated);
    let loaded = fixture.ok("config.get", Value::Null);
    assert_eq!(loaded["sleep_time"], "22:30");
    let error = fixture.err("config.set", json!({"op_id":"op-config-empty"}));
    assert_eq!(error.data.unwrap().kind, "validation");

    let add = fixture.ok(
        "item.add",
        json!({"op_id":"op-add","title":"Export me","block":"fajr"}),
    );
    assert_eq!(add["revision"], 1);

    let exported = fixture.ok("export", Value::Null);
    assert_eq!(exported["format"], "salah-export");
    assert_eq!(exported["version"], 3);
    assert!(exported["items"][0]["version"].is_i64());

    let imported = fixture.ok("import", json!({"op_id":"op-import","data":exported}));
    assert_eq!(imported["summary"]["unchanged"], 1);

    let replay = fixture.ok("import", json!({"op_id":"op-import","data":exported}));
    assert_eq!(replay, imported);

    let mut invalid = exported.clone();
    invalid["format"] = json!("not-salah");
    let error = fixture.err("import", json!({"op_id":"op-bad-import","data":invalid}));
    assert_eq!(error.data.unwrap().kind, "validation");
}

#[test]
fn replay_after_later_edits_returns_byte_identical_original_results() {
    let mut fixture = Fixture::new();
    fixture.handshake();

    let added = fixture.ok(
        "item.add",
        json!({"op_id":"op-replay-add","title":"Original","block":"fajr"}),
    );
    let id = item_id(&added).to_string();
    let added_bytes = serde_json::to_string(&added).unwrap();

    // The complete response is stored in the same transaction as the write.
    let applied = fixture.store.applied_op("op-replay-add").unwrap().unwrap();
    let stored: Value = serde_json::from_str(&applied.result_json).unwrap();
    assert_eq!(stored["rpc"]["changed"]["items"][0]["title"], "Original");

    let first_edit = fixture.ok(
        "item.edit",
        json!({
            "op_id":"op-replay-edit-1",
            "id":id,
            "expected_version":1,
            "title":"First edit"
        }),
    );
    let first_edit_bytes = serde_json::to_string(&first_edit).unwrap();

    // A later edit changes the current projection and version. Replaying the
    // earlier ops must not recompute rows from that newer state.
    let second_edit = fixture.ok(
        "item.edit",
        json!({
            "op_id":"op-replay-edit-2",
            "id":id,
            "expected_version":2,
            "title":"Second edit"
        }),
    );
    assert_eq!(item_version(&second_edit), 3);

    let replay_add = fixture.ok(
        "item.add",
        json!({"op_id":"op-replay-add","title":"Different retry body","block":"asr"}),
    );
    assert_eq!(serde_json::to_string(&replay_add).unwrap(), added_bytes);

    let replay_edit = fixture.ok(
        "item.edit",
        json!({
            "op_id":"op-replay-edit-1",
            "id":id,
            "title":"Different retry body"
        }),
    );
    assert_eq!(
        serde_json::to_string(&replay_edit).unwrap(),
        first_edit_bytes
    );
    assert_eq!(replay_edit["changed"]["items"][0]["title"], "First edit");
    assert_eq!(replay_edit["changed"]["items"][0]["version"], 2);
}

#[test]
fn timer_start_reports_the_previously_running_item_too() {
    let mut fixture = Fixture::new();
    fixture.handshake();

    let first = fixture.ok(
        "item.add",
        json!({"op_id":"op-timer-a","title":"First timer","measure":"minutes"}),
    );
    let first_id = item_id(&first).to_string();
    let second = fixture.ok(
        "item.add",
        json!({"op_id":"op-timer-b","title":"Second timer","measure":"minutes"}),
    );
    let second_id = item_id(&second).to_string();

    let started_first = fixture.ok(
        "timer.start",
        json!({"op_id":"op-start-a","item_id":first_id}),
    );
    assert_eq!(
        started_first["changed"]["items"].as_array().unwrap().len(),
        1
    );

    let started_second = fixture.ok(
        "timer.start",
        json!({"op_id":"op-start-b","item_id":second_id}),
    );
    let rows = started_second["changed"]["items"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "both the started and stopped items change");
    let stopped = rows.iter().find(|row| row["id"] == first_id).unwrap();
    let running = rows.iter().find(|row| row["id"] == second_id).unwrap();
    assert!(stopped["timer_started_at"].is_null());
    assert!(running["timer_started_at"].is_string());
}

#[test]
fn item_move_by_after_id_resolves_the_anchor_in_the_move_transaction() {
    let mut fixture = Fixture::new();
    fixture.handshake();

    let a = fixture.ok(
        "item.add",
        json!({"op_id":"op-move-a","title":"A","block":"fajr"}),
    );
    let a_id = item_id(&a).to_string();
    let b = fixture.ok(
        "item.add",
        json!({"op_id":"op-move-b","title":"B","block":"fajr"}),
    );
    let b_id = item_id(&b).to_string();
    let c = fixture.ok(
        "item.add",
        json!({"op_id":"op-move-c","title":"C","block":"fajr"}),
    );
    let c_id = item_id(&c).to_string();

    let moved = fixture.ok(
        "item.move",
        json!({
            "op_id":"op-move-after",
            "id":a_id,
            "block":"fajr",
            "after_id":c_id
        }),
    );
    let order: Vec<&str> = moved["changed"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(order, [b_id.as_str(), c_id.as_str(), a_id.as_str()]);
}

#[test]
fn every_protocol_method_has_a_happy_path_and_an_error() {
    let mut fixture = Fixture::new();
    fixture.handshake();

    let check = |fixture: &mut Fixture, method: &str, good: Value, bad: Value| {
        let result = fixture.ok(method, good);
        assert!(!result.is_null(), "{method} returned null");
        let error = fixture.err(method, bad);
        assert!(
            error.code == INVALID_PARAMS || error.code <= -32000,
            "{method} returned an unexpected error: {error:?}"
        );
    };

    check(
        &mut fixture,
        "hello",
        json!({"protocol":[1],"client":"again"}),
        json!({"protocol":[2],"client":"old"}),
    );
    check(&mut fixture, "status.get", json!({}), json!([]));
    check(
        &mut fixture,
        "day.get",
        json!({"date":"2031-05-14"}),
        json!({"date":"not-a-date"}),
    );
    check(&mut fixture, "items.list", json!({}), json!([]));
    check(
        &mut fixture,
        "stats.get",
        json!({"days":7}),
        json!({"from":"2031-05-01"}),
    );
    check(
        &mut fixture,
        "history.get",
        json!({}),
        json!({"to":"not-a-date"}),
    );
    check(
        &mut fixture,
        "item.parse",
        json!({"text":"read 20m after isha daily"}),
        json!({}),
    );
    check(
        &mut fixture,
        "item.preview",
        json!({
            "block":"asr","repeat":"daily","measure":"check","date":"2031-05-14"
        }),
        json!({}),
    );

    let added = fixture.ok(
        "item.add",
        json!({"op_id":"op-all-add","title":"All methods","block":"fajr","measure":"minutes"}),
    );
    let id = item_id(&added).to_string();
    check(
        &mut fixture,
        "item.add",
        json!({"op_id":"op-all-add-2","title":"Second"}),
        json!({"op_id":"op-all-add-bad"}),
    );
    check(
        &mut fixture,
        "item.edit",
        json!({"op_id":"op-all-edit","id":id,"title":"Renamed"}),
        json!({"op_id":"op-all-edit-bad","id":id,"expected_version":999,"title":"Stale"}),
    );
    let extra = fixture.ok(
        "item.add",
        json!({"op_id":"op-all-extra","title":"Extra","block":"fajr"}),
    );
    let extra_id = item_id(&extra).to_string();
    check(
        &mut fixture,
        "item.move",
        json!({"op_id":"op-all-move","id":extra_id,"block":"fajr","after_id":id}),
        json!({"op_id":"op-all-move-bad","id":extra_id,"block":"fajr","after_id":id,"before_id":id}),
    );
    check(
        &mut fixture,
        "item.remove",
        json!({"op_id":"op-all-remove","id":extra_id}),
        json!({"op_id":"op-all-remove-bad","id":"missing"}),
    );
    check(
        &mut fixture,
        "entry.done",
        json!({"op_id":"op-all-done","item_id":id,"date":"2031-05-14"}),
        json!({"op_id":"op-all-done-bad","item_id":"missing"}),
    );
    check(
        &mut fixture,
        "entry.undo",
        json!({"op_id":"op-all-undo","item_id":id,"date":"2031-05-14"}),
        json!({"op_id":"op-all-undo-bad","item_id":"missing"}),
    );
    check(
        &mut fixture,
        "entry.log",
        json!({"op_id":"op-all-log","item_id":id,"kind":"minutes","value":5}),
        json!({"op_id":"op-all-log-bad","item_id":id,"value":0}),
    );
    check(
        &mut fixture,
        "timer.start",
        json!({"op_id":"op-all-start","item_id":id}),
        json!({"op_id":"op-all-start-bad","item_id":"missing"}),
    );
    check(
        &mut fixture,
        "timer.stop",
        json!({"op_id":"op-all-stop","item_id":id}),
        json!({"op_id":"op-all-stop-bad","item_id":"missing"}),
    );
    check(
        &mut fixture,
        "prayer.mark",
        json!({"op_id":"op-all-mark","prayer":"dhuhr","date":"2031-05-14"}),
        json!({"op_id":"op-all-mark-bad","prayer":"sleep"}),
    );
    check(
        &mut fixture,
        "prayer.unmark",
        json!({"op_id":"op-all-unmark","prayer":"dhuhr","date":"2031-05-14"}),
        json!({"op_id":"op-all-unmark-bad","prayer":"sleep"}),
    );
    check(&mut fixture, "config.get", json!({}), json!([]));
    check(
        &mut fixture,
        "config.set",
        json!({"op_id":"op-all-config","sleep_time":"22:15"}),
        json!({"op_id":"op-all-config-bad"}),
    );
    let exported = fixture.ok("export", json!({}));
    check(&mut fixture, "export", json!({}), json!([]));
    check(
        &mut fixture,
        "import",
        json!({"op_id":"op-all-import","data":exported}),
        json!({"op_id":"op-all-import-bad","data":{"format":"not-salah"}}),
    );
    check(
        &mut fixture,
        "subscribe",
        json!({"topics":["changes","clock"],"after_revision":0}),
        json!({"topics":["unknown"]}),
    );
}
