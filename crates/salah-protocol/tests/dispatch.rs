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
    assert_eq!(hello["capabilities"], json!(["timer.v1"]));

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
