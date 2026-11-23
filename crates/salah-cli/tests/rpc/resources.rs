use super::phase2::{added, bad, good};
use super::{DATE, Fixtures, Harness, LATER, NOW, Session, id};
use serde_json::{Value, json};

fn item_row<'a>(day: &'a Value, id: &str) -> &'a Value {
    day["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|block| block["items"].as_array().unwrap())
        .find(|item| item["id"] == id)
        .unwrap()
}

#[test]
fn resources_follow_their_parent_and_status_shows_only_due_open_reminders() {
    let harness = Harness::new();
    let mut rpc = Session::open(&harness, NOW);
    let mut fixtures = Fixtures::default();
    let task = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"task","title":"Read","block":"fajr"}),
    );
    let backlog = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"backlog","title":"Order books","backlog":true}),
    );
    let project = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"project","name":"Study"}),
    );
    let epic = added(
        &mut rpc,
        "epic.add",
        "epics",
        json!({"op_id":"epic","project_id":project,"name":"Reading list"}),
    );
    let link_result = fixtures.check(
        "resource_add_link",
        rpc.request(
            "resource.add",
            json!({"op_id":"link","item_id":task,"kind":"link","title":"Reading",
               "url":"https://example.org/books?q=first"}),
        ),
    );
    let link = id(&link_result, "resources");
    assert_eq!(
        good(&mut rpc, "resource.add", json!({"op_id":"link"})),
        link_result
    );
    let exact = fixtures.check(
        "resource_add_reminder",
        rpc.request(
            "resource.add",
            json!({"op_id":"exact","item_id":task,"kind":"reminder","title":"Check opening times",
               "remind_at":"2031-05-14T09:30:00+05:00"}),
        ),
    );
    let exact_id = id(&exact, "resources");
    let older = added(
        &mut rpc,
        "resource.add",
        "resources",
        json!({"op_id":"older","epic_id":epic,"kind":"reminder","title":"Find the book",
               "remind_at":"2031-05-14T04:25:00Z"}),
    );
    let future = added(
        &mut rpc,
        "resource.add",
        "resources",
        json!({"op_id":"future","item_id":backlog,"kind":"reminder","title":"Order online",
               "remind_at":"2031-05-14T04:40:00Z"}),
    );
    let note = added(
        &mut rpc,
        "resource.add",
        "resources",
        json!({"op_id":"note","item_id":task,"kind":"note","body":"First line\nSecond line"}),
    );

    let listed = fixtures.check(
        "resources_list",
        rpc.request("resources.list", json!({"item_id":task})),
    );
    assert_eq!(
        listed
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].clone())
            .collect::<Vec<_>>(),
        [json!(link), json!(exact_id), json!(note)]
    );
    let on_epic = good(&mut rpc, "resources.list", json!({"epic_id":epic}));
    assert_eq!(on_epic[0]["id"], older);
    assert_eq!(on_epic[0]["epic_id"], epic);
    assert!(on_epic[0]["item_id"].is_null());

    let status = fixtures.check("status_reminders", rpc.request("status.get", json!({})));
    let reminders = status["reminders"].as_array().unwrap();
    assert_eq!(reminders.len(), 2);
    assert_eq!(reminders[0]["id"], older);
    assert_eq!(reminders[0]["epic_name"], "Reading list");
    assert_eq!(reminders[0]["overdue"], true);
    assert_eq!(reminders[1]["id"], exact_id);
    assert_eq!(reminders[1]["item_title"], "Read");
    assert_eq!(reminders[1]["overdue"], false);
    let day = good(&mut rpc, "day.get", json!({"date":DATE}));
    assert_eq!(
        item_row(&day, &task)["resources"],
        json!({"notes":1,"links":1,"reminders":1})
    );
    assert_eq!(
        good(&mut rpc, "backlog.list", json!({}))[0]["resources"],
        json!({"notes":0,"links":0,"reminders":1})
    );

    let done = fixtures.check(
        "resource_edit_done",
        rpc.request(
            "resource.edit",
            json!({"op_id":"done","id":exact_id,"done":true,"expected_version":1}),
        ),
    );
    assert_eq!(done["changed"]["resources"][0]["version"], 2);
    assert_eq!(done["changed"]["resources"][0]["done"], true);
    assert_eq!(
        good(&mut rpc, "resource.edit", json!({"op_id":"done"})),
        done
    );
    let day = good(&mut rpc, "day.get", json!({"date":DATE}));
    assert_eq!(item_row(&day, &task)["resources"]["reminders"], 0);
    assert_eq!(
        good(&mut rpc, "status.get", json!({}))["reminders"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let removed = good(
        &mut rpc,
        "resource.remove",
        json!({"op_id":"remove-link","id":link,"expected_version":1}),
    );
    assert_eq!(removed["changed"]["deleted"], json!([link]));
    assert_eq!(
        good(&mut rpc, "resource.remove", json!({"op_id":"remove-link"})),
        removed
    );
    let day = good(&mut rpc, "day.get", json!({"date":DATE}));
    assert_eq!(item_row(&day, &task)["resources"]["links"], 0);
    good(
        &mut rpc,
        "epic.remove",
        json!({"op_id":"remove-epic","id":epic}),
    );
    assert_eq!(
        good(&mut rpc, "status.get", json!({}))["reminders"],
        json!([])
    );

    let mut later = Session::open(&harness, LATER);
    let status = good(&mut later, "status.get", json!({}));
    assert_eq!(status["reminders"].as_array().unwrap().len(), 1);
    assert_eq!(status["reminders"][0]["id"], future);
    good(
        &mut later,
        "item.remove",
        json!({"op_id":"remove-backlog","id":backlog}),
    );
    assert_eq!(
        good(&mut later, "status.get", json!({}))["reminders"],
        json!([])
    );
}

#[test]
fn resources_validate_owners_fields_and_versions_before_writing() {
    let harness = Harness::new();
    let mut rpc = Session::open(&harness, NOW);
    let task = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"item","title":"Read"}),
    );
    for (params, field) in [
        (json!({"kind":"note","body":"Text"}), "item_id"),
        (
            json!({"item_id":task,"epic_id":"also","kind":"note","body":"Text"}),
            "item_id",
        ),
        (json!({"item_id":task,"kind":"note","body":"  "}), "body"),
        (
            json!({"item_id":task,"kind":"link","url":"file:///etc/passwd"}),
            "url",
        ),
        (
            json!({"item_id":task,"kind":"reminder","title":"No date"}),
            "remind_at",
        ),
    ] {
        let mut params = params;
        params["op_id"] = json!("unapplied");
        bad(&mut rpc, "resource.add", params, "validation", Some(field));
    }
    bad(
        &mut rpc,
        "resources.list",
        json!({"item_id":task,"epic_id":"also"}),
        "validation",
        Some("item_id"),
    );
    bad(
        &mut rpc,
        "resources.list",
        json!({"epic_id":"missing"}),
        "not_found",
        None,
    );
    assert_eq!(
        good(&mut rpc, "resources.list", json!({"item_id":task})),
        json!([])
    );
    let note = added(
        &mut rpc,
        "resource.add",
        "resources",
        json!({"op_id":"unapplied","item_id":task,"kind":"note","title":"Temporary title","body":"Text"}),
    );
    for (params, kind, field) in [
        (json!({"body":null}), "validation", Some("body")),
        (json!({"done":true}), "validation", Some("done")),
        (
            json!({"title":"Stale","expected_version":9}),
            "conflict",
            None,
        ),
    ] {
        let mut params = params;
        params["id"] = json!(note);
        params["op_id"] = json!("edit");
        bad(&mut rpc, "resource.edit", params, kind, field);
    }
    let edited = good(
        &mut rpc,
        "resource.edit",
        json!({"op_id":"edit","id":note,"title":null,"expected_version":1}),
    );
    assert_eq!(edited["revision"], 3);
    assert!(edited["changed"]["resources"][0]["title"].is_null());
    assert_eq!(edited["changed"]["resources"][0]["body"], "Text");
}
