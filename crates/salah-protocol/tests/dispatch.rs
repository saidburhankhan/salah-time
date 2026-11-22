//! Dispatcher invariants: the handshake, framing faults, params validation,
//! idempotent replay, version conflicts, and the missing-metrics error.
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
fn stopping_without_required_metrics_names_the_missing_ones() {
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
    let error = fixture.err(
        "session.stop",
        json!({"op_id":"op-stop","values":{"quality":3}}),
    );
    let data = error.data.unwrap();
    assert_eq!(
        (data.kind.as_str(), data.field.as_deref()),
        ("validation", Some("values"))
    );
    assert_eq!(data.missing, Some(vec!["focus".to_owned()]));
    let stopped = fixture.ok(
        "session.stop",
        json!({"op_id":"op-stop","values":{"focus":4,"quality":3}}),
    );
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
    let config = fixture.err("config.set", json!({"op_id":"op-1","sleep_time":"22:00"}));
    assert_eq!(config.data.unwrap().kind, "validation");
}
