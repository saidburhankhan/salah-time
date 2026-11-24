use super::phase2::{added, bad, good};
use super::{Fixtures, Harness, NOW, Session};
use serde_json::json;
use std::collections::BTreeMap;
use std::process::Command;

#[test]
fn defaults_explicit_tags_and_retagged_history() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let tag = added(
        &mut rpc,
        "tag.add",
        "tags",
        json!({"op_id":"t","name":"Study","color":"#223344"}),
    );
    let p = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"p","name":"School","tag_ids":[tag]}),
    );
    let inherited = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"inherit","title":"Review","project_id":p}),
    );
    let empty = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"empty","title":"Break","project_id":p,"tag_ids":[],"backlog":true}),
    );
    let mut fixtures = Fixtures::with_ids(&[&tag]);
    let tagged = fixtures.check(
        "item_add_tags",
        rpc.request(
            "item.add",
            json!({"op_id":"task","title":"Read","backlog":true,"tag_ids":[tag]}),
        ),
    );
    let loose = &tagged["changed"]["items"][0];
    assert_eq!(loose["tag_ids"], json!([tag]));
    let loose = loose["id"].as_str().unwrap();
    for (op, target, min) in [
        ("work", json!({"item_id":inherited}), 10),
        ("break", json!({"item_id":empty}), 20),
        ("read", json!({"item_id":loose}), 30),
        ("project", json!({"project_id":p}), 40),
    ] {
        let mut params = target;
        params["op_id"] = json!(op);
        params["minutes"] = json!(min);
        good(&mut rpc, "session.log", params);
    }
    let table = fixtures.check(
        "stats_table_tag_items",
        rpc.request("stats.table", json!({"days":1,"group":"tag"})),
    );
    assert_eq!(table["rows"][0]["minutes"], 80);
    assert_eq!(table["rows"][1]["minutes"], 20);
    assert_eq!(table["totals"]["minutes"], 100);
    assert_eq!(
        good(&mut rpc, "backlog.list", json!({"tag_id":tag}))[0]["id"],
        loose
    );
    assert_eq!(
        good(&mut rpc, "backlog.list", json!({"tag_id":"none"}))[0]["id"],
        empty
    );

    good(
        &mut rpc,
        "project.edit",
        json!({"id":p,"op_id":"defaults","tag_ids":[]}),
    );
    let moved = good(
        &mut rpc,
        "item.edit",
        json!({"id":inherited,"op_id":"move","project_id":null}),
    );
    assert_eq!(moved["changed"]["items"][0]["tag_ids"], json!([tag]));
    good(
        &mut rpc,
        "item.edit",
        json!({"id":loose,"op_id":"untag","tag_ids":[]}),
    );
    let tagged = good(&mut rpc, "stats.get", json!({"tag_id":tag,"days":1}));
    assert_eq!(tagged["totals"]["minutes"], 10);
    let listed = good(
        &mut rpc,
        "sessions.list",
        json!({"tag_id":"none","days":1,"with_note":false}),
    );
    assert_eq!(listed["rows"].as_array().unwrap().len(), 3);
    let trends = good(
        &mut rpc,
        "stats.series",
        json!({"tag_id":tag,"days":1,"split":"tag","measures":["minutes"]}),
    );
    assert_eq!(trends["series"][0]["points"][0]["value"], 10.0);
    let alpha = added(
        &mut rpc,
        "tag.add",
        "tags",
        json!({"op_id":"alpha","name":"Alpha","color":"#334455"}),
    );
    let both = good(
        &mut rpc,
        "item.edit",
        json!({"id":inherited,"op_id":"both","tag_ids":[tag,alpha]}),
    );
    // Item views list tags in name order, like the tag breakdowns.
    assert_eq!(both["changed"]["items"][0]["tag_ids"], json!([alpha, tag]));
    let removed = good(&mut rpc, "tag.remove", json!({"op_id":"remove","id":tag}));
    let retagged = removed["changed"]["items"].as_array().unwrap();
    assert_eq!(retagged.len(), 1);
    assert_eq!(retagged[0]["id"], inherited);
    assert_eq!(retagged[0]["tag_ids"], json!([alpha]));
    assert_eq!(removed["changed"]["deleted"], json!([tag]));
    let items = good(&mut rpc, "items.list", json!({}));
    let task = items
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == inherited)
        .unwrap();
    assert_eq!(task["tag_ids"], json!([alpha]));
    assert_eq!(task["version"], 4);
    bad(
        &mut rpc,
        "item.edit",
        json!({"op_id":"removed","id":loose,"tag_ids":[tag]}),
        "not_found",
        None,
    );
    bad(
        &mut rpc,
        "item.add",
        json!({"op_id":"unknown","title":"Draft","tag_ids":["missing"]}),
        "not_found",
        None,
    );
}

fn sqlite(h: &Harness, sql: &str) -> String {
    let out = Command::new("sqlite3")
        .arg("-batch")
        .arg("-bail")
        .arg(&h.data)
        .arg(format!("\n{sql}"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn migrated_v4_tag_totals_through_rpc() {
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../salah-store/src/migrations");
    for moved in [false, true] {
        let h = Harness::new();
        for file in [
            "001_schema.sql",
            "tests/tracking_v1.sql",
            "002_history.sql",
            "003_tracking_ui.sql",
            "004_epics_backlog_resources.sql",
            "tests/tracking_v4.sql",
        ] {
            sqlite(&h, &std::fs::read_to_string(migrations.join(file)).unwrap());
        }
        sqlite(&h, "PRAGMA user_version=4");
        if moved {
            sqlite(&h,"UPDATE items SET project_id='n', epic_id=NULL, metrics_custom=0, version=2 WHERE id='course';
                       DELETE FROM item_metrics WHERE item_id='course'");
        }
        let before=sqlite(&h,"SELECT COALESCE(t.tag_id,'none'), count(*), SUM(s.minutes)
            FROM sessions s LEFT JOIN (SELECT pt.* FROM project_tags pt JOIN tags t ON t.id=pt.tag_id WHERE t.deleted_at IS NULL) t
            ON t.project_id=s.project_id WHERE s.deleted_at IS NULL AND (s.started_at IS NULL OR s.ended_at IS NOT NULL)
            GROUP BY 1 ORDER BY 1");
        assert_eq!(before, "none|2|35\nt|7|152\nu|4|115\n");
        let mut rpc = Session::open(&h, NOW);
        let table = good(
            &mut rpc,
            "stats.table",
            json!({"from":"2031-05-10","to":"2031-05-14","group":"tag"}),
        );
        let rows: BTreeMap<_, _> = table["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["key"].as_str().unwrap(),
                    (
                        r["sessions"].as_i64().unwrap(),
                        r["minutes"].as_i64().unwrap(),
                    ),
                )
            })
            .collect();
        let after: String = rows
            .iter()
            .map(|(key, (n, m))| format!("{key}|{n}|{m}\n"))
            .collect();
        if moved {
            // The lecture and its tick follow the task to an untagged project.
            assert_eq!(after, "none|4|85\nt|5|102\nu|2|65\n");
        } else {
            assert_eq!(after, before);
        }
        assert_eq!(table["totals"]["minutes"], 187);
        assert_eq!(table["totals"]["sessions"], 9);
        assert_eq!(
            sqlite(&h, "PRAGMA user_version").trim(),
            salah_store::schema_version().to_string()
        );
    }
}
