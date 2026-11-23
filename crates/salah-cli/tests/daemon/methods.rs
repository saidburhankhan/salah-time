//! One success and one error for every protocol method, over the socket.

use super::{Client, DATE, Daemon};
use serde_json::json;

#[test]
fn every_protocol_method_has_a_success_and_an_error_over_the_socket() {
    let daemon = Daemon::start();
    let mut client = daemon.client();
    client.hello();
    method_success_and_error(&mut client);
}

fn method_success_and_error(client: &mut Client) {
    // hello is already done; a second hello is valid, a newer protocol is not.
    let hello = client.good("hello", json!({"protocol":[1],"client":"again"}));
    assert_eq!(hello["protocol"], 1);
    assert!(
        hello["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "tracking.v1")
    );
    assert_eq!(
        client.bad("hello", json!({"protocol":[2],"client":"old"})),
        -32602
    );

    assert!(client.good("status.get", json!({}))["day"].is_string());
    assert_eq!(client.bad("status.get", json!([])), -32602);

    assert_eq!(client.good("day.get", json!({"date": DATE}))["day"], DATE);
    assert_eq!(client.bad("day.get", json!({"date":"not-a-date"})), -32602);

    assert!(client.good("items.list", json!({})).is_array());
    assert_eq!(client.bad("items.list", json!([])), -32602);

    assert_eq!(
        client.good("stats.get", json!({"days":7}))["days"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    assert_eq!(
        client.bad("stats.get", json!({"from":"2031-05-01"})),
        -32001
    );
    assert_eq!(
        client.good("stats.table", json!({"days":7}))["group"],
        "week"
    );
    assert_eq!(client.bad("stats.table", json!({"days":0})), -32001);
    assert_eq!(
        client.good("stats.series", json!({"days":7,"measures":["minutes"]}))["series"][0]["key"],
        "all"
    );
    assert_eq!(client.bad("stats.series", json!({"days":7})), -32001);
    assert!(client.good("sessions.list", json!({"days":7}))["rows"].is_array());
    assert_eq!(client.bad("sessions.list", json!({"limit":0})), -32001);

    assert_eq!(
        client.good("item.parse", json!({"text":"read 20m after isha daily"}))["block"],
        "isha"
    );
    assert_eq!(client.bad("item.parse", json!({})), -32602);

    let preview = client.good(
        "item.preview",
        json!({"block":"asr","repeat":"weekly","weekdays":["mon","wed","fri"],"planned_minutes":20,"date":DATE}),
    );
    assert_eq!(preview["schedule_text"], "Mon, Wed, Fri");
    assert_eq!(client.bad("item.preview", json!({"block":"asr"})), -32602);

    let metrics = client.good("metrics.list", json!({}));
    assert_eq!(metrics[0]["id"], "focus");
    assert_eq!(
        client.bad("metrics.list", json!({"archived":"yes"})),
        -32602
    );
    let metric = client.good(
        "metric.add",
        json!({"op_id":"s4-metric","name":"Pages","unit":"pages","kind":"count"}),
    )["changed"]["metrics"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        client.bad(
            "metric.add",
            json!({"op_id":"s4-metric-bad","name":"Mood","kind":"scale"})
        ),
        -32001
    );
    assert_eq!(
        client.good(
            "metric.edit",
            json!({"op_id":"s4-metric-edit","id":metric,"unit":"pp"})
        )["changed"]["metrics"][0]["unit"],
        "pp"
    );
    assert_eq!(
        client.bad(
            "metric.edit",
            json!({"op_id":"s4-metric-edit-bad","id":"focus","archived":true})
        ),
        -32001
    );

    let tag = client.good(
        "tag.add",
        json!({"op_id":"s4-tag","name":"Deen","color":"#00aa00"}),
    )["changed"]["tags"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        client.bad(
            "tag.add",
            json!({"op_id":"s4-tag-bad","name":"Deen","color":"#00aa00"})
        ),
        -32001
    );
    assert_eq!(
        client.good(
            "tag.edit",
            json!({"op_id":"s4-tag-edit","id":tag,"name":"Faith"})
        )["changed"]["tags"][0]["name"],
        "Faith"
    );
    assert_eq!(
        client.bad(
            "tag.edit",
            json!({"op_id":"s4-tag-edit-bad","id":tag,"color":"red"})
        ),
        -32001
    );
    assert_eq!(
        client
            .good("tags.list", json!({}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(client.bad("tags.list", json!([])), -32602);

    let project = client.good(
        "project.add",
        json!({"op_id":"s4-project","name":"Quran","tag_ids":[tag]}),
    )["changed"]["projects"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        client.bad(
            "project.add",
            json!({"op_id":"s4-project-bad","name":"X","tag_ids":["missing"]})
        ),
        -32002
    );
    assert_eq!(
        client.good(
            "project.edit",
            json!({"op_id":"s4-project-edit","id":project,"color":"#AA0000"})
        )["changed"]["projects"][0]["color"],
        "#aa0000"
    );
    assert_eq!(
        client.bad(
            "project.edit",
            json!({"op_id":"s4-project-edit-bad","id":project,"expected_version":99,"name":"Y"})
        ),
        -32004
    );
    assert_eq!(
        client.good("projects.list", json!({}))[0]["metrics"][0]["metric_id"],
        "focus"
    );
    assert_eq!(client.bad("projects.list", json!([])), -32602);

    let added = client.good(
        "item.add",
        json!({"op_id":"s4-add","title":"All methods","block":"fajr","project_id":project,"planned_minutes":30}),
    );
    let row = &added["changed"]["items"][0];
    let id = row["id"].as_str().unwrap().to_string();
    assert_eq!(
        (row["version"].clone(), row["schedule_text"].clone()),
        (json!(1), json!("Once"))
    );
    assert_eq!(
        client.bad("item.add", json!({"op_id":"s4-add-bad"})),
        -32001
    );
    check_resources(client, &id);
    let extra = client.good(
        "item.add",
        json!({"op_id":"s4-extra","title":"Extra","block":"fajr"}),
    )["changed"]["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        client.good(
            "item.edit",
            json!({"op_id":"s4-edit","id":id,"title":"Renamed"})
        )["changed"]["items"][0]["title"],
        "Renamed"
    );
    assert_eq!(
        client.bad(
            "item.edit",
            json!({"op_id":"s4-edit-bad","id":id,"expected_version":999,"title":"Stale"})
        ),
        -32004
    );
    assert!(
        client.good(
            "item.move",
            json!({"op_id":"s4-move","id":extra,"block":"fajr","after_id":id})
        )["changed"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == extra)
    );
    assert_eq!(
        client.bad(
            "item.move",
            json!({"op_id":"s4-move-bad","id":extra,"block":"fajr","after_id":id,"before_id":id})
        ),
        -32001
    );
    assert_eq!(
        client.good("item.remove", json!({"op_id":"s4-remove","id":extra}))["changed"]["deleted"],
        json!([extra])
    );
    assert_eq!(
        client.bad(
            "item.remove",
            json!({"op_id":"s4-remove-bad","id":"missing"})
        ),
        -32002
    );
    let shelved = client.good(
        "item.add",
        json!({"op_id":"s4-backlog-add","title":"Someday","backlog":true}),
    )["changed"]["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(client.good("backlog.list", json!({}))[0]["id"], shelved);
    assert_eq!(client.bad("backlog.list", json!([])), -32602);
    assert_eq!(
        client.good(
            "item.schedule",
            json!({"op_id":"s4-schedule","id":shelved,"day":DATE,"block":"isha"})
        )["changed"]["items"][0]["block"],
        "isha"
    );
    assert_eq!(
        client.bad(
            "item.schedule",
            json!({"op_id":"s4-schedule-bad","id":shelved})
        ),
        -32602
    );
    assert_eq!(
        client.good("item.backlog", json!({"op_id":"s4-shelve","id":shelved}))["changed"]["items"]
            [0]["backlog"],
        true
    );
    assert_eq!(
        client.bad(
            "item.backlog",
            json!({"op_id":"s4-shelve-bad","id":"missing"})
        ),
        -32002
    );
    assert!(client.good("epics.list", json!({})).is_array());
    assert_eq!(client.bad("epics.list", json!({"status":"done"})), -32602);

    let started = client.good("session.start", json!({"op_id":"s4-start","item_id":id}));
    let session = started["changed"]["sessions"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(started["changed"]["sessions"][0]["running"], true);
    assert_eq!(
        client.bad(
            "session.start",
            json!({"op_id":"s4-start-bad","item_id":id})
        ),
        -32004
    );
    let status = client.good("status.get", json!({}));
    assert_eq!(status["running"]["id"], json!(session));
    assert_eq!(status["prompt"][1]["metric_id"], "quality");
    assert_eq!(
        client.bad(
            "session.stop",
            json!({"op_id":"s4-stop-bad","values":{"focus":6}})
        ),
        -32001
    );
    assert_eq!(
        client.good("session.stop", json!({"op_id":"s4-stop"}))["changed"]["sessions"][0]["minutes"],
        1
    );
    let logged = client.good(
        "session.log",
        json!({"op_id":"s4-log","project_id":project,"minutes":15,"values":{"focus":3,"quality":3}}),
    )["changed"]["sessions"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        client.bad(
            "session.log",
            json!({"op_id":"s4-log-bad","project_id":project,"minutes":0})
        ),
        -32001
    );
    assert_eq!(
        client.good(
            "session.edit",
            json!({"op_id":"s4-session-edit","id":logged,"minutes":20})
        )["changed"]["sessions"][0]["minutes"],
        20
    );
    assert_eq!(
        client.bad(
            "session.edit",
            json!({"op_id":"s4-session-edit-bad","id":"missing"})
        ),
        -32002
    );
    assert_eq!(
        client.good(
            "session.remove",
            json!({"op_id":"s4-session-remove","id":logged})
        )["changed"]["deleted"],
        json!([logged])
    );
    assert_eq!(
        client.bad(
            "session.remove",
            json!({"op_id":"s4-session-remove-bad","id":logged})
        ),
        -32002
    );

    assert_eq!(
        client.good("tag.remove", json!({"op_id":"s4-tag-remove","id":tag}))["changed"]["projects"]
            [0]["tags"],
        json!([])
    );
    assert_eq!(
        client.bad("tag.remove", json!({"op_id":"s4-tag-remove-bad","id":tag})),
        -32002
    );

    assert_eq!(
        client.good(
            "prayer.mark",
            json!({"op_id":"s4-mark","prayer":"dhuhr","date":DATE})
        )["changed"]["marks"][0]["prayer"],
        "dhuhr"
    );
    assert_eq!(
        client.bad(
            "prayer.mark",
            json!({"op_id":"s4-mark-bad","prayer":"sleep"})
        ),
        -32001
    );
    assert_eq!(
        client.good(
            "prayer.unmark",
            json!({"op_id":"s4-unmark","prayer":"dhuhr","date":DATE})
        )["changed"]["marks"][0]["prayer"],
        "dhuhr"
    );
    assert_eq!(
        client.bad(
            "prayer.unmark",
            json!({"op_id":"s4-unmark-bad","prayer":"sleep"})
        ),
        -32001
    );

    assert_eq!(client.good("config.get", json!({}))["method"], "mwl");
    assert_eq!(client.bad("config.get", json!([])), -32602);
    assert_eq!(
        client.good(
            "config.set",
            json!({"op_id":"s4-config","sleep_time":"22:15"})
        )["sleep_time"],
        "22:15"
    );
    assert_eq!(
        client.bad("config.set", json!({"op_id":"s4-config-bad"})),
        -32001
    );

    for removed in [
        "entry.done",
        "timer.start",
        "history.get",
        "export",
        "import",
    ] {
        assert_eq!(
            client.bad(removed, json!({})),
            -32601,
            "{removed} must be gone"
        );
    }

    let subscribed = client.good(
        "subscribe",
        json!({"topics":["changes"],"after_revision":0}),
    );
    assert!(subscribed["revision"].as_i64().unwrap() > 0);
    assert_eq!(
        client.bad("subscribe", json!({"topics":["unknown"]})),
        -32001
    );
}

fn check_resources(client: &mut Client, item_id: &str) {
    let reminder = client.good(
        "resource.add",
        json!({"op_id":"add-reminder","item_id":item_id,"kind":"reminder",
               "remind_at":"2031-05-14T04:30:00Z"}),
    );
    let id = reminder["changed"]["resources"][0]["id"].as_str().unwrap();
    assert_eq!(
        client.good("resources.list", json!({"item_id":item_id}))[0]["id"],
        id
    );
    assert_eq!(client.bad("resources.list", json!({})), -32001);
    assert_eq!(
        client.bad(
            "resource.add",
            json!({"op_id":"invalid-note","item_id":item_id,"kind":"note"})
        ),
        -32001
    );
    assert_eq!(
        client.good("resource.edit", json!({"op_id":"done","id":id,"done":true}))["changed"]["resources"]
            [0]["done"],
        true
    );
    assert_eq!(
        client.bad(
            "resource.edit",
            json!({"op_id":"stale-resource","id":id,"expected_version":1})
        ),
        -32004
    );
    assert_eq!(
        client.good(
            "resource.remove",
            json!({"op_id":"remove-resource","id":id})
        )["changed"]["deleted"],
        json!([id])
    );
    assert_eq!(
        client.bad("resource.remove", json!({"op_id":"remove-again","id":id})),
        -32002
    );
}
