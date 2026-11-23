//! Socket lifecycle, permissions, notifications, and concurrent writers.

use super::{DATE, Daemon, WAIT, wait_timeout};
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::Stdio;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

#[test]
fn socket_permissions_and_handshake() {
    let daemon = Daemon::start();
    let dir = daemon.socket.parent().unwrap();
    assert_eq!(
        fs::metadata(dir).unwrap().permissions().mode() & 0o777,
        0o700,
        "runtime directory must be private"
    );
    assert_eq!(
        fs::metadata(&daemon.socket).unwrap().permissions().mode() & 0o777,
        0o600,
        "socket must be user-only"
    );

    let mut client = daemon.client();
    assert_eq!(client.bad("status.get", json!({})), -32600);
    let hello = client.hello();
    assert_eq!(hello["result"]["protocol"], 1);
    assert!(
        hello["result"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "events.v1")
    );
    assert!(client.good("status.get", json!({}))["day"].is_string());
}

#[test]
fn clock_boundary_is_pushed_when_the_next_boundary_changes() {
    let daemon = Daemon::start();
    let mut subscriber = daemon.client();
    subscriber.hello();
    let subscribed = subscriber.good("subscribe", json!({"topics":["clock"],"after_revision":0}));
    assert!(subscribed["revision"].as_i64().is_some());

    // The subscribe response carries an immediate snapshot with a server
    // instant, so no local boundary maths is needed.
    let initial = subscriber.wait_method("clock.boundary");
    assert_eq!(initial["params"]["day"], DATE);
    assert_eq!(initial["params"]["next"]["id"], "dhuhr");
    assert_eq!(initial["params"]["next"]["at"], "2031-05-14T07:19:00Z");
    assert!(
        initial["params"]["next"]["remaining_seconds"]
            .as_i64()
            .unwrap()
            > 0
    );

    // Move the sleep boundary to one minute after the frozen clock. The
    // scheduler must notice and push the new next boundary.
    let mut writer = daemon.client();
    writer.hello();
    writer.good(
        "config.set",
        json!({"op_id":"s4-clock-boundary","sleep_time":"09:31"}),
    );
    let deadline = Instant::now() + WAIT;
    loop {
        assert!(
            Instant::now() < deadline,
            "no clock.boundary for the changed sleep boundary"
        );
        let value = subscriber.recv_value();
        if value.get("method").and_then(Value::as_str) != Some("clock.boundary") {
            continue;
        }
        if value["params"]["next"]["id"] != "sleep" {
            continue;
        }
        assert_eq!(value["params"]["next"]["remaining_seconds"], 60);
        assert_eq!(value["params"]["next"]["at"], "2031-05-14T04:31:00Z");
        break;
    }
}

#[test]
fn subscribers_receive_changed_for_socket_and_plain_cli_writes() {
    let daemon = Daemon::start();
    let mut subscriber = daemon.client();
    subscriber.hello();
    let subscribed = subscriber.good("subscribe", json!({"topics":["changes"]}));
    let base_revision = subscribed["revision"].as_i64().unwrap();

    let mut writer = daemon.client();
    writer.hello();
    let added = writer.good(
        "item.add",
        json!({"op_id":"s4-sub-socket","title":"From socket","block":"asr","date":DATE}),
    );
    let revision = added["revision"].as_i64().unwrap();
    assert!(revision > base_revision);
    let notification = subscriber.wait_changed_after(revision);
    assert_eq!(notification["params"]["revision"], revision);
    assert!(
        notification["params"]["scopes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|scope| scope == "items")
    );

    let output = daemon.cli(&[
        "item", "add", "From CLI", "--block", "asr", "--date", DATE, "--json",
    ]);
    assert!(
        output.status.success(),
        "CLI add failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let notification = subscriber.wait_changed_after(revision + 1);
    assert!(notification["params"]["revision"].as_i64().unwrap() > revision);

    let item = added["changed"]["items"][0]["id"].as_str().unwrap();
    let resource = writer.good(
        "resource.add",
        json!({"op_id":"s4-sub-resource","item_id":item,"kind":"note","body":"Keep this"}),
    );
    let resource_revision = resource["revision"].as_i64().unwrap();
    let notification = subscriber.wait_changed_after(resource_revision);
    assert_eq!(notification["params"]["scopes"], json!(["resources"]));
    assert_eq!(notification["params"]["op_id"], "s4-sub-resource");
}

#[test]
fn concurrent_writers_keep_revisions_gapless() {
    const WRITERS: usize = 4;
    const ADDS: usize = 5;
    let daemon = Arc::new(Daemon::start());
    let barrier = Arc::new(Barrier::new(WRITERS));
    let mut handles = Vec::new();
    for writer in 0..WRITERS {
        let daemon = Arc::clone(&daemon);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            let mut client = daemon.client();
            client.hello();
            barrier.wait();
            let mut revisions = Vec::new();
            for add in 0..ADDS {
                let result = client.good(
                    "item.add",
                    json!({
                        "op_id": format!("s4-concurrent-{writer}-{add}"),
                        "title": format!("writer {writer} add {add}"),
                        "block": "asr",
                        "date": DATE,
                    }),
                );
                revisions.push(result["revision"].as_i64().unwrap());
            }
            revisions
        }));
    }
    let mut revisions = Vec::new();
    for handle in handles {
        revisions.extend(handle.join().unwrap());
    }
    revisions.sort_unstable();
    assert_eq!(
        revisions,
        (1..=(WRITERS * ADDS) as i64).collect::<Vec<_>>(),
        "one serialised writer must allocate every revision exactly once"
    );

    let mut reader = daemon.client();
    reader.hello();
    assert_eq!(
        reader
            .good("items.list", json!({}))
            .as_array()
            .unwrap()
            .len(),
        WRITERS * ADDS
    );
}

#[test]
fn a_second_daemon_refuses_to_steal_the_live_socket() {
    let daemon = Daemon::start();
    let mut second = daemon
        .base_command()
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let status = wait_timeout(&mut second, WAIT);
    let status = match status {
        Some(status) => status,
        None => {
            let _ = second.kill();
            let _ = second.wait();
            panic!("second daemon kept running instead of refusing the live socket");
        }
    };
    assert!(!status.success());
    let bytes = second
        .stderr
        .take()
        .map(|mut stderr| {
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut stderr, &mut bytes).unwrap();
            bytes
        })
        .unwrap_or_default();
    let error = String::from_utf8_lossy(&bytes);
    assert!(
        error.contains("already listening"),
        "unexpected second-daemon error: {error}"
    );

    let mut client = daemon.client();
    client.hello();
    assert!(client.good("status.get", json!({}))["day"].is_string());
}

#[test]
fn a_stale_socket_file_is_replaced_and_sigterm_removes_it() {
    let mut daemon = Daemon::prepare();
    let dir = daemon.socket.parent().unwrap();
    fs::create_dir_all(dir).unwrap();
    // Bind and drop a listener to leave a real socket file without a server.
    drop(std::os::unix::net::UnixListener::bind(&daemon.socket).unwrap());
    assert!(daemon.socket.exists());

    daemon.spawn();
    daemon.wait_for_socket();
    let mut client = daemon.client();
    client.hello();
    assert!(client.good("status.get", json!({}))["day"].is_string());

    let status = daemon.terminate();
    assert!(
        status.success(),
        "SIGTERM shutdown should succeed: {status}"
    );
    daemon.wait_until_socket_removed();
}

#[test]
fn rpc_socket_bridges_stdio_to_the_daemon() {
    let daemon = Daemon::start();
    let mut child = daemon
        .base_command()
        .args(["rpc", "--socket"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":1,"method":"hello","params":{{"protocol":[1],"client":"bridge"}}}}"#
        )
        .unwrap();
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "bridge failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let line = String::from_utf8_lossy(&output.stdout);
    let response: Value = serde_json::from_str(line.lines().next().unwrap()).unwrap();
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["protocol"], 1);
}
