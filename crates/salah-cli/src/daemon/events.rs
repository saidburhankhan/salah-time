use super::{SHUTDOWN, Shared, WATCH_INTERVAL};
use salah_protocol::{ChangeScope, ChangedNotification};
use salah_store::Store;
use serde_json::Value;
use std::sync::{Arc, atomic::Ordering, mpsc};
use std::thread;

#[derive(Clone, Copy)]
pub(super) enum Topic {
    Changes,
    Clock,
}

struct Subscriber {
    id: u64,
    sender: mpsc::Sender<Vec<u8>>,
    changes: bool,
    clock: bool,
}

#[derive(Default)]
pub(super) struct Subscribers(Vec<Subscriber>);

impl Subscribers {
    /// Params have already been validated by the dispatcher.
    pub(super) fn subscribe(
        &mut self,
        id: u64,
        params: &Value,
        sender: mpsc::Sender<Vec<u8>>,
    ) -> bool {
        let topics = params["topics"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let clock = topics.iter().any(|t| t == "clock");
        let changes = topics.is_empty() || topics.iter().any(|t| t == "changes");
        self.unsubscribe(id);
        self.0.push(Subscriber {
            id,
            sender,
            changes,
            clock,
        });
        clock
    }

    pub(super) fn unsubscribe(&mut self, id: u64) {
        self.0.retain(|s| s.id != id);
    }

    pub(super) fn broadcast(&mut self, topic: Topic, line: &str) {
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\n');
        self.0.retain(|s| {
            let subscribed = match topic {
                Topic::Changes => s.changes,
                Topic::Clock => s.clock,
            };
            !subscribed || s.sender.send(bytes.clone()).is_ok()
        });
    }
}

pub(super) fn watch(store: Store, shared: Arc<Shared>, mut last_version: i64, mut last_seq: i64) {
    while !SHUTDOWN.load(Ordering::SeqCst) {
        thread::sleep(WATCH_INTERVAL);
        let result = store.data_version().and_then(|version| {
            if version != last_version {
                drain(&store, &mut last_seq, 10_000, |event| {
                    if let Ok(line) = serde_json::to_string(&event.envelope()) {
                        shared.broadcast(Topic::Changes, &line);
                    }
                })?;
                last_version = version;
            }
            Ok(())
        });
        if let Err(error) = result {
            eprintln!("salah: read changes: {error}");
        }
    }
}

/// Publish a revision once, even if its changes cross a query page. Advance
/// the cursor only after publishing a complete revision.
fn drain(
    store: &Store,
    last_seq: &mut i64,
    limit: usize,
    mut emit: impl FnMut(ChangedNotification),
) -> Result<(), salah_domain::error::Error> {
    let mut cursor = *last_seq;
    let mut pending: Option<ChangedNotification> = None;
    loop {
        let page = store.changes_since(cursor, limit)?;
        let at_end = page.len() < limit;
        for change in page {
            if pending
                .as_ref()
                .is_some_and(|event| event.revision != change.revision)
                && let Some(event) = pending.take()
            {
                emit(event);
                *last_seq = cursor;
            }
            let event = pending.get_or_insert_with(|| ChangedNotification {
                revision: change.revision,
                days: Vec::new(),
                scopes: Vec::new(),
                op_id: change.op_id,
                actor: "daemon".into(),
            });
            if let Some(scope) = ChangeScope::of_entity(&change.entity)
                && !event.scopes.contains(&scope)
            {
                event.scopes.push(scope);
            }
            cursor = change.seq;
        }
        if at_end {
            if let Some(event) = pending {
                emit(event);
            }
            *last_seq = cursor;
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tracking_mutations_publish_all_scopes_once_per_revision() {
        use salah_core::config::Config;
        use salah_domain::{
            error::Error,
            runtime::{SystemClock, UuidV4},
        };
        use salah_protocol::{ConfigRepository, Context, Dispatcher};
        struct Settings;
        impl ConfigRepository for Settings {
            fn load(&self) -> Result<Config, Error> {
                Ok(Config {
                    method: "mwl".into(),
                    ..Default::default()
                })
            }
            fn save(&self, _: &Config) -> Result<(), Error> {
                unreachable!()
            }
        }

        let mut store = Store::open_in_memory().unwrap();
        let mut dispatcher = Dispatcher::new();
        dispatcher.accept_hello();
        let mut cursor = 0;
        let mut call = |method: &str, mut body: Value, scopes: &[&str]| {
            body["op_id"] = json!(method);
            let result = dispatcher
                .call(
                    method,
                    &body,
                    &mut Context {
                        store: &mut store,
                        config: &Settings,
                        clock: &SystemClock,
                        ids: &UuidV4,
                    },
                )
                .unwrap();
            let mut events = Vec::new();
            // One row per page puts the tag deletion and its detached
            // project on different pages of the same transaction.
            drain(&store, &mut cursor, 1, |event| events.push(event)).unwrap();
            assert_eq!(events.len(), 1, "{method}");
            assert_eq!(json!(events[0].revision), result["revision"]);
            assert_eq!(events[0].op_id.as_deref(), Some(method));
            assert_eq!(json!(events[0].scopes), json!(scopes));
            result["changed"][scopes[0]][0]["id"].clone()
        };
        let tag = call(
            "tag.add",
            json!({"name":"Study","color":"#334455"}),
            &["tags"],
        );
        call("tag.edit", json!({"id":tag,"name":"Learn"}), &["tags"]);
        let metric = call(
            "metric.add",
            json!({"name":"Pages","kind":"count"}),
            &["metrics"],
        );
        call(
            "metric.edit",
            json!({"id":metric,"unit":"pages"}),
            &["metrics"],
        );
        let project = call(
            "project.add",
            json!({"name":"Book","metrics":[],"tag_ids":[tag]}),
            &["projects"],
        );
        let epic = call(
            "epic.add",
            json!({"project_id":project, "name":"Chapter"}),
            &["epics"],
        );
        let resource = call(
            "resource.add",
            json!({"epic_id":epic,"kind":"note","body":"Outline"}),
            &["resources"],
        );
        call(
            "resource.edit",
            json!({"id":resource,"body":"Second draft"}),
            &["resources"],
        );
        call("resource.remove", json!({"id":resource}), &["resources"]);
        call(
            "epic.edit",
            json!({"id":epic, "status":"finished"}),
            &["epics"],
        );
        call("epic.remove", json!({"id":epic}), &["epics"]);
        call(
            "project.edit",
            json!({"id":project,"note":"Read"}),
            &["projects"],
        );
        call(
            "session.start",
            json!({"project_id":project}),
            &["sessions"],
        );
        call("session.stop", json!({}), &["sessions"]);
        let session = call("session.log", json!({"project_id":project}), &["sessions"]);
        call(
            "session.edit",
            json!({"id":session,"minutes":15}),
            &["sessions"],
        );
        call("session.remove", json!({"id":session}), &["sessions"]);
        call("tag.remove", json!({"id":tag}), &["projects", "tags"]);
    }

    #[test]
    fn subscriptions_deliver_only_requested_topics_including_after_resubscribe() {
        let mut subscribers = Subscribers::default();
        let (tx, rx) = mpsc::channel();
        subscribers.subscribe(1, &json!({}), tx.clone());
        subscribers.broadcast(Topic::Clock, "clock");
        subscribers.broadcast(Topic::Changes, "changed");
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), [b"changed\n".to_vec()]);
        subscribers.subscribe(1, &json!({"topics":["clock"]}), tx);
        subscribers.broadcast(Topic::Changes, "changed");
        subscribers.broadcast(Topic::Clock, "clock");
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), [b"clock\n".to_vec()]);
    }
}
