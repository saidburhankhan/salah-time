//! Dispatcher invariants: the handshake, framing faults, params validation,
//! idempotent replay, version conflicts, and tracking behaviour.
//! Every method's happy path runs over the socket in `salah-cli/tests/daemon.rs`
//! and against the golden fixtures in `salah-cli/tests/rpc.rs`.

use chrono::{TimeZone, Utc};
use salah_core::Store;
use salah_core::config::Config;
use salah_domain::error::Error;
use salah_domain::runtime::{FixedClock, SequenceIds};
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
}

fn first_id(result: &Value, kind: &str) -> String {
    result["changed"][kind][0]["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn handshake_is_required_and_negotiates_protocol_one() {
    let mut fixture = Fixture::new();
    assert_eq!(fixture.err("status.get", Value::Null).code, INVALID_REQUEST);
    assert_eq!(
        fixture.err("hello", json!({"protocol":[2]})).code,
        INVALID_PARAMS
    );
    let hello = fixture.handshake();
    assert_eq!(hello["protocol"], 1);
    assert_eq!(
        hello["capabilities"],
        json!(["tracking.v1", "events.v1", "clock.v1"])
    );
    assert_eq!(
        fixture.err("not.a.method", Value::Null).code,
        METHOD_NOT_FOUND
    );
}

#[test]
fn framing_faults_are_jsonrpc_errors() {
    let mut fixture = Fixture::new();
    assert!(fixture.line("   ").is_none());
    let fault = fixture.line("{not json").unwrap();
    assert_eq!(fault.error.unwrap().code, PARSE_ERROR);
    let oversized = "x".repeat(salah_protocol::MAX_LINE_BYTES + 1);
    assert_eq!(
        fixture.line(&oversized).unwrap().error.unwrap().code,
        PARSE_ERROR
    );
}

#[test]
fn every_method_rejects_non_object_params_with_a_validation_kind() {
    let mut fixture = Fixture::new();
    fixture.handshake();
    for method in Dispatcher::methods() {
        let error = fixture.err(method, json!([1]));
        assert_eq!(error.code, INVALID_PARAMS, "{method}");
        assert_eq!(error.data.unwrap().kind, "validation", "{method}");
    }
}

#[test]
fn mutations_are_exactly_the_writing_methods() {
    let writes: Vec<&str> = Dispatcher::methods()
        .filter(|method| Dispatcher::is_mutation(method))
        .collect();
    assert!(writes.iter().all(|m| m.contains(".add")
        || m.contains(".edit")
        || m.contains(".remove")
        || m.contains(".move")
        || *m == "item.schedule"
        || *m == "item.backlog"
        || *m == "goal.end"
        || *m == "review.save"
        || m.starts_with("session.")
        || m.starts_with("prayer.")
        || *m == "config.set"));
    assert!(!Dispatcher::is_mutation("status.get"));
    for removed in [
        "entry.done",
        "entry.undo",
        "entry.log",
        "timer.start",
        "timer.stop",
        "export",
        "import",
        "history.get",
    ] {
        assert!(
            Dispatcher::methods().all(|m| m != removed),
            "{removed} is still served"
        );
    }
}

#[test]
fn a_replayed_op_id_returns_the_original_result_and_writes_nothing() {
    let mut fixture = Fixture::new();
    fixture.handshake();
    let added = fixture.ok(
        "item.add",
        json!({"op_id":"op-add","title":"Read","block":"fajr","planned_minutes":20}),
    );
    let id = first_id(&added, "items");
    fixture.ok(
        "item.edit",
        json!({"op_id":"op-edit","id":id,"title":"Recite"}),
    );
    let revision = fixture.store.revision().unwrap();

    let replay = fixture.ok(
        "item.add",
        json!({"op_id":"op-add","title":"Different body"}),
    );
    assert_eq!(
        replay.to_string(),
        added.to_string(),
        "byte-identical replay"
    );
    assert_eq!(fixture.store.revision().unwrap(), revision);
    assert_eq!(
        fixture
            .ok("items.list", Value::Null)
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a_stale_expected_version_is_a_conflict_that_writes_nothing() {
    let mut fixture = Fixture::new();
    fixture.handshake();
    let project = first_id(
        &fixture.ok("project.add", json!({"op_id":"op-project","name":"Quran"})),
        "projects",
    );
    fixture.ok(
        "project.edit",
        json!({"op_id":"op-rename","id":project,"name":"Qur'an","expected_version":1}),
    );
    let revision = fixture.store.revision().unwrap();
    let error = fixture.err(
        "project.edit",
        json!({"op_id":"op-stale","id":project,"name":"Stale","expected_version":1}),
    );
    assert_eq!(error.data.unwrap().kind, "conflict");
    assert_eq!(fixture.store.revision().unwrap(), revision);
    assert!(
        fixture.store.applied_op("op-stale").unwrap().is_none(),
        "a failed op can be retried"
    );
}

#[test]
fn stopping_without_values_succeeds() {
    let mut fixture = Fixture::new();
    fixture.handshake();
    let project = first_id(
        &fixture.ok("project.add", json!({"op_id":"op-project","name":"Quran"})),
        "projects",
    );
    fixture.ok(
        "session.start",
        json!({"op_id":"op-start","project_id":project}),
    );
    let stopped = fixture.ok("session.stop", json!({"op_id":"op-stop"}));
    assert_eq!(stopped["changed"]["sessions"][0]["values"], json!({}));
    assert_eq!(stopped["changed"]["sessions"][0]["running"], false);
}

#[test]
fn a_config_set_replay_keeps_the_first_body() {
    let mut fixture = Fixture::new();
    fixture.handshake();
    let updated = fixture.ok(
        "config.set",
        json!({"op_id":"op-config","sleep_time":"22:30"}),
    );
    let replay = fixture.ok(
        "config.set",
        json!({"op_id":"op-config","sleep_time":"21:00"}),
    );
    assert_eq!(replay, updated);
    assert_eq!(
        fixture.ok("config.set", json!({"op_id":"op-config"})),
        updated
    );
    assert_eq!(fixture.ok("config.get", Value::Null)["sleep_time"], "22:30");
    let error = fixture.err("config.set", json!({"op_id":"op-config-empty"}));
    assert_eq!(error.data.unwrap().kind, "validation");
}

#[test]
fn an_op_id_must_be_non_empty_and_belongs_to_one_method() {
    let mut fixture = Fixture::new();
    fixture.handshake();
    let empty = fixture.err(
        "tag.add",
        json!({"op_id":" ","name":"Deen","color":"#00aa00"}),
    );
    let data = empty.data.unwrap();
    assert_eq!(
        (data.kind.as_str(), data.field.as_deref()),
        ("validation", Some("op_id"))
    );

    fixture.ok(
        "tag.add",
        json!({"op_id":"op-1","name":"Deen","color":"#00aa00"}),
    );
    let reused = fixture.err("project.add", json!({"op_id":"op-1","name":"Quran"}));
    assert_eq!(reused.data.unwrap().kind, "validation");
    assert!(reused.message.contains("tag.add"), "{}", reused.message);
    assert!(
        fixture
            .ok("projects.list", Value::Null)
            .as_array()
            .unwrap()
            .is_empty()
    );
    let malformed = fixture.err("project.add", json!({"op_id":"op-1"}));
    assert_eq!(malformed.data.unwrap().field.as_deref(), Some("op_id"));
    let config = fixture.err("config.set", json!({"op_id":"op-1","sleep_time":"22:00"}));
    assert_eq!(config.data.unwrap().kind, "validation");
}

#[test]
fn routine_timer_prompts_for_optional_builtin_metrics_and_stops_without_them() {
    let mut f = Fixture::new();
    f.handshake();
    assert_eq!(f.ok("status.get", json!({}))["prompt"], json!([]));
    let item = first_id(
        &f.ok("item.add", json!({"op_id":"item","title":"Read"})),
        "items",
    );
    f.ok("session.start", json!({"op_id":"start","item_id":item}));
    let status = f.ok("status.get", json!({}));
    let prompt = status["prompt"].as_array().unwrap();
    assert_eq!(prompt.len(), 3);
    for (row, id) in prompt.iter().zip(["focus", "quality", "distractions"]) {
        assert_eq!(row["metric_id"], id);
        assert_eq!(row["required"], false);
    }
    f.ok("session.stop", json!({"op_id":"stop"}));
}

#[test]
fn settings_changes_preserve_even_unread_past_days() {
    let mut f = Fixture::new();
    f.handshake();
    let expected = {
        let mut reference = Fixture::new();
        reference.handshake();
        reference.ok("day.get", json!({"date":"2031-05-12"}))
    };
    f.ok(
        "config.set",
        json!({"timezone_offset": -5.0, "sleep_time":"22:00","longitude":-74.0}),
    );
    let past = f.ok("day.get", json!({"date":"2031-05-12"}));
    assert_eq!(past, expected);
    assert_eq!(f.ok("day.get", json!({}))["sleep"]["time"], "22:00");
    let changes = f.store.changes_since(0, 10).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].entity, "config");
}

#[test]
fn running_session_keeps_partial_ratings_and_notes_until_stop() {
    let mut f = Fixture::new();
    f.handshake();
    let project = first_id(
        &f.ok("project.add", json!({"op_id":"p","name":"Book"})),
        "projects",
    );
    let session = first_id(
        &f.ok("session.start", json!({"op_id":"s","project_id":project})),
        "sessions",
    );
    f.ok(
        "session.edit",
        json!({"op_id":"e","id":session,"values":{"focus":4},"note":"chapter 1"}),
    );
    let stopped = f.ok(
        "session.stop",
        json!({"op_id":"stop","values":{"quality":3}}),
    );
    let row = &stopped["changed"]["sessions"][0];
    assert_eq!(row["values"], json!({"focus":4.0,"quality":3.0}));
    assert_eq!(row["note"], "chapter 1");
}

#[test]
fn edits_removals_and_prayer_marks_freeze_the_day_they_write() {
    let mut f = Fixture::new();
    f.handshake();
    let project = first_id(
        &f.ok(
            "project.add",
            json!({"op_id":"p","name":"Book","metrics":[]}),
        ),
        "projects",
    );
    let mut sessions = Vec::new();
    for day in [12, 13] {
        f.clock = FixedClock::new(Utc.with_ymd_and_hms(2031, 5, day, 8, 0, 0).unwrap());
        let logged = f.ok(
            "session.log",
            json!({"op_id":format!("log-{day}"),"project_id":project}),
        );
        sessions.push(first_id(&logged, "sessions"));
    }
    f.clock = FixedClock::new(Utc.with_ymd_and_hms(2031, 5, 14, 8, 0, 0).unwrap());
    f.ok(
        "session.edit",
        json!({"op_id":"edit","id":sessions[0],"minutes":10}),
    );
    f.ok("session.remove", json!({"op_id":"remove","id":sessions[1]}));
    f.ok(
        "prayer.mark",
        json!({"op_id":"mark","date":"2031-05-11","prayer":"fajr"}),
    );
    for day in [11, 12, 13] {
        assert!(
            f.store
                .day_written(chrono::NaiveDate::from_ymd_opt(2031, 5, day).unwrap())
                .unwrap()
        );
    }
}

#[test]
fn a_large_finite_count_has_a_finite_minute_weighted_average() {
    let mut f = Fixture::new();
    f.handshake();
    let metric = first_id(
        &f.ok(
            "metric.add",
            json!({"op_id":"m","name":"Count","kind":"count","rollup":"avg"}),
        ),
        "metrics",
    );
    let project = first_id(
        &f.ok(
            "project.add",
            json!({"op_id":"p","name":"Book","metrics":[]}),
        ),
        "projects",
    );
    for (key, count) in [("a", 1e308_f64), ("b", 0.0)] {
        f.ok("session.log", json!({"op_id":key,"project_id":project,"minutes":10,"values":{(metric.as_str()):count}}));
    }
    let stats = f.ok("stats.get", json!({"days":1}));
    assert_eq!(stats["projects"][0]["metrics"][&metric], json!(5e307));
}

#[test]
fn project_filters_keep_plan_line_sessions_distinct_from_unplanned_minutes() {
    let mut f = Fixture::new();
    f.handshake();
    let a = first_id(
        &f.ok("project.add", json!({"op_id":"a","name":"A","metrics":[]})),
        "projects",
    );
    let b = first_id(
        &f.ok("project.add", json!({"op_id":"b","name":"B","metrics":[]})),
        "projects",
    );
    let item = first_id(&f.ok("item.add", json!({"op_id":"item","title":"Read","project_id":a,"planned_minutes":15,"block":"asr"})), "items");
    f.ok(
        "session.log",
        json!({"op_id":"planned","item_id":item,"minutes":10,"block":"dhuhr"}),
    );
    f.ok(
        "session.log",
        json!({"op_id":"extra","project_id":a,"minutes":5,"block":"dhuhr"}),
    );
    f.ok(
        "item.edit",
        json!({"op_id":"move","id":item,"project_id":b}),
    );
    let stats = f.ok("stats.get", json!({"days":1,"project_id":a}));
    let day = &stats["days"][0];
    assert_eq!(day["minutes"], 5);
    assert_eq!(day["planned_minutes"], 0);
    assert!(day["fulfilment_ratio"].is_null());
    assert_eq!(day["unplanned_minutes"], 5);
    let plan = f.ok("day.get", json!({}));
    let block = plan["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["start"] == "dhuhr")
        .unwrap();
    assert_eq!(block["actual_minutes"], 10);
    assert_eq!(block["unplanned_minutes"], 5);
}

#[test]
fn a_tick_stops_only_its_item_timer_and_replays_both_rows_once() {
    let mut f = Fixture::new();
    f.handshake();
    let project = first_id(
        &f.ok("project.add", json!({"op_id":"p","name":"Study"})),
        "projects",
    );
    let item = first_id(&f.ok("item.add", json!({"op_id":"i","title":"Read","project_id":project,"planned_minutes":30,"repeat":"daily"})), "items");
    let other = first_id(
        &f.ok(
            "item.add",
            json!({"op_id":"other","title":"Walk","project_id":project}),
        ),
        "items",
    );
    let timer = first_id(
        &f.ok("session.start", json!({"op_id":"start","item_id":item})),
        "sessions",
    );

    for body in [
        json!({"op_id":"other-tick","item_id":other}),
        json!({"op_id":"project-tick","project_id":project}),
        json!({"op_id":"log-time","item_id":item,"minutes":5}),
    ] {
        assert_eq!(
            f.ok("session.log", body)["changed"]["sessions"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
    let status = f.ok("status.get", json!({}));
    assert_eq!(status["running"]["id"], timer);
    let row = status["planner"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == item)
        .unwrap();
    assert_eq!(row["done"], false);

    // 09:30 to 23:00 local time. The tick belongs to the following day.
    f.clock = FixedClock::new(Utc.with_ymd_and_hms(2031, 5, 15, 4, 30, 0).unwrap());
    let revision = f.store.revision().unwrap();
    let seq = f.store.last_change_seq().unwrap();
    f.err(
        "session.log",
        json!({"op_id":"done","item_id":item,"values":{"focus":6}}),
    );
    assert_eq!(f.store.revision().unwrap(), revision);
    assert_eq!(f.ok("status.get", json!({}))["running"]["id"], timer);

    let result = f.ok(
        "session.log",
        json!({"op_id":"done","item_id":item,"values":{"focus":4},"note":"finished"}),
    );
    let rows = result["changed"]["sessions"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let (tick, stopped) = (&rows[0], &rows[1]);
    assert_eq!(tick["day"], "2031-05-15");
    assert!(tick["minutes"].is_null());
    assert_eq!(tick["values"], json!({"focus":4.0}));
    assert_eq!(tick["note"], "finished");
    assert_eq!(stopped["id"], timer);
    assert_eq!(stopped["ended_at"], "2031-05-14T18:00:00Z");
    assert_eq!(stopped["minutes"], 810);
    assert_eq!(stopped["values"], json!({}));
    assert!(stopped["note"].is_null());
    assert!(f.ok("status.get", json!({}))["running"].is_null());
    assert_eq!(f.ok("session.log", json!({"op_id":"done"})), result);
    assert_eq!(f.store.revision().unwrap(), revision + 1);
    let changes = f.store.changes_since(seq, 10).unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].entity_id, timer);
    assert_eq!(changes[1].entity_id, tick["id"].as_str().unwrap());
    assert!(
        changes
            .iter()
            .all(|c| c.revision == revision + 1 && c.op_id.as_deref() == Some("done"))
    );

    let today = f.ok("day.get", json!({}));
    assert_eq!(today["fulfilment"]["done_items"], 1);
    assert_eq!(today["fulfilment"]["actual_minutes"], 0);
    let yesterday = f.ok("day.get", json!({"date":"2031-05-14"}));
    assert_eq!(yesterday["fulfilment"]["done_items"], 1); // only Walk
    assert_eq!(yesterday["fulfilment"]["actual_minutes"], 815);
    f.ok("session.remove", json!({"op_id":"untick","id":tick["id"]}));
    assert_eq!(f.ok("day.get", json!({}))["fulfilment"]["done_items"], 0);
    assert_eq!(f.ok("day.get", json!({"date":"2031-05-14"})), yesterday);
}
