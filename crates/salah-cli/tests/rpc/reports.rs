//! `stats.table` and `sessions.list` over stdio, with their golden fixtures.

use super::phase2::{added, bad, good};
use super::{Fixtures, Harness, LATER, NOW, Session};
use serde_json::{Value, json};

/// Quran (tag Study) with the task Read, Gym, and the unassigned task Walk.
/// Sessions: Read 30 min on Thu 8 May (W19), Gym 20 min on Mon 12 May, Walk
/// 15 min on Tue 13 May, a Read tick and a noted Read timer today (W20).
struct Seeded {
    tag: String,
    quran: String,
    gym: String,
    read: String,
    walk: String,
}

fn seed(h: &Harness) -> Seeded {
    let mut rpc = Session::open(h, NOW);
    let tag = added(
        &mut rpc,
        "tag.add",
        "tags",
        json!({"op_id":"tag","name":"Study","color":"#223344"}),
    );
    let quran = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"quran","name":"Quran","color":"#335577","tag_ids":[tag]}),
    );
    let gym = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"gym","name":"Gym","color":"#775533"}),
    );
    let read = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"read","title":"Read","project_id":quran,"planned_minutes":30}),
    );
    let walk = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"walk","title":"Walk","planned_minutes":15}),
    );
    for (op, params) in [
        (
            "log-read",
            json!({"item_id":read,"date":"2031-05-08","block":"dhuhr","minutes":30,
                   "values":{"focus":4,"quality":5,"distractions":2},"note":"Āyat 1–10 of al-Mulk"}),
        ),
        (
            "log-gym",
            json!({"project_id":gym,"date":"2031-05-12","block":"asr","minutes":20,
                   "values":{"focus":2,"distractions":1}}),
        ),
        (
            "log-walk",
            json!({"item_id":walk,"date":"2031-05-13","block":"maghrib","minutes":15,
                   "note":"Evening walk"}),
        ),
        ("tick-read", json!({"item_id":read,"values":{"focus":5}})),
    ] {
        let mut params = params;
        params["op_id"] = json!(op);
        good(&mut rpc, "session.log", params);
    }
    good(
        &mut rpc,
        "session.start",
        json!({"op_id":"start","item_id":read}),
    );
    good(
        &mut rpc,
        "session.note",
        json!({"op_id":"entry","text":"Halfway through"}),
    );
    drop(rpc);
    let mut later = Session::open(h, LATER);
    good(
        &mut later,
        "session.stop",
        json!({"op_id":"stop","values":{"focus":3}}),
    );
    Seeded {
        tag,
        quran,
        gym,
        read,
        walk,
    }
}

fn keys(table: &Value) -> Vec<&str> {
    table["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["key"].as_str().unwrap())
        .collect()
}

fn titles(list: &Value) -> Vec<Value> {
    list["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["item"]["title"].clone())
        .collect()
}

#[test]
fn stats_table_groups_weeks_and_projects_with_unassigned_rows() {
    let h = Harness::new();
    let s = seed(&h);
    let mut rpc = Session::open(&h, NOW);
    let mut fixtures = Fixtures::default();
    let range = json!({"from":"2031-05-01","to":"2031-05-14"});
    let week = fixtures.check(
        "stats_table_week",
        rpc.request("stats.table", range.clone()),
    );
    assert_eq!(week["group"], "week");
    assert_eq!(keys(&week), ["2031-W18", "2031-W19", "2031-W20"]);
    assert_eq!(week["rows"][0]["sessions"], 0, "an empty week stays");
    assert_eq!(week["rows"][0]["from"], "2031-05-01");
    assert_eq!(week["rows"][2]["to"], "2031-05-14");
    // 30 + 20 + 15 + the 25-minute timer; four timed sessions and a tick.
    assert_eq!(week["totals"]["minutes"], 90);
    assert_eq!(week["totals"]["sessions"], 5);
    let columns: Vec<&str> = week["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        columns,
        [
            "minutes",
            "sessions",
            "focus",
            "quality",
            "distractions",
            "distractions_per_hour"
        ]
    );
    // W20: Gym 20 min focus 2, Walk 15 min, the tick focus 5, the timer 25 min focus 3.
    let w20 = &week["rows"][2]["values"];
    let focus = (2.0 * 20.0 + 5.0 + 3.0 * 25.0) / 46.0;
    assert!((w20["focus"].as_f64().unwrap() - focus).abs() < 1e-9);
    assert_eq!(w20["quality"], Value::Null);
    assert_eq!(w20["distractions"], 1.0);
    assert!((w20["distractions_per_hour"].as_f64().unwrap() - 60.0 / 60.0).abs() < 1e-9);

    let mut params = range.clone();
    params["group"] = json!("project");
    let project = fixtures.check("stats_table_project", rpc.request("stats.table", params));
    assert_eq!(keys(&project), [s.quran.as_str(), s.gym.as_str(), "none"]);
    assert_eq!(project["rows"][2]["label"], "No project");
    assert_eq!(project["rows"][2]["color"], Value::Null);
    assert_eq!(project["rows"][0]["minutes"], 55);
    assert_eq!(project["rows"][0]["sessions"], 3);

    let mut params = range.clone();
    params["group"] = json!("tag");
    let tags = good(&mut rpc, "stats.table", params);
    assert_eq!(keys(&tags), [s.tag.as_str(), "none"]);
    assert_eq!(tags["rows"][1]["label"], "No tag");
    assert_eq!(tags["totals"]["minutes"], 90);

    let mut params = range.clone();
    params["group"] = json!("item");
    let tasks = good(&mut rpc, "stats.table", params);
    assert_eq!(keys(&tasks), [s.read.as_str(), "none", s.walk.as_str()]);
    assert_eq!(tasks["rows"][1]["label"], "No task");

    let mut params = range.clone();
    params["group"] = json!("day");
    let days = good(&mut rpc, "stats.table", params);
    assert_eq!(days["rows"].as_array().unwrap().len(), 14);

    let unassigned = good(
        &mut rpc,
        "stats.table",
        json!({"days":30,"group":"item","project_id":"none"}),
    );
    assert_eq!(keys(&unassigned), [s.walk.as_str()]);
    assert_eq!(unassigned["filter"]["project_id"], "none");
    let tagged = good(
        &mut rpc,
        "stats.table",
        json!({"days":30,"group":"project","tag_id":s.tag}),
    );
    assert_eq!(keys(&tagged), [s.quran.as_str()]);

    bad(
        &mut rpc,
        "stats.table",
        json!({"days":7,"group":"month"}),
        "validation",
        None,
    );
    bad(
        &mut rpc,
        "stats.table",
        json!({"days":731}),
        "validation",
        Some("days"),
    );
    bad(
        &mut rpc,
        "stats.table",
        json!({"from":"2031-05-14","to":"2031-05-01"}),
        "validation",
        Some("to"),
    );
    bad(
        &mut rpc,
        "stats.table",
        json!({"days":7,"project_id":"missing"}),
        "not_found",
        None,
    );
}

#[test]
fn sessions_list_filters_in_sql_and_pages_by_cursor() {
    let h = Harness::new();
    let s = seed(&h);
    let mut rpc = Session::open(&h, NOW);
    let mut fixtures = Fixtures::default();
    let listed = fixtures.check(
        "sessions_list",
        rpc.request("sessions.list", json!({"days":30})),
    );
    // Notes only, newest first: the timer (an entry), the walk, the read log.
    assert_eq!(
        titles(&listed),
        [json!("Read"), json!("Walk"), json!("Read")]
    );
    assert_eq!(listed["next"], Value::Null);
    let timer = &listed["rows"][0];
    assert_eq!(timer["notes"][0]["text"], "Halfway through");
    assert_eq!(timer["project"]["name"], "Quran");
    assert_eq!(timer["day"], "2031-05-14");
    assert_eq!(listed["rows"][1]["project"], Value::Null);

    let filtered = fixtures.check(
        "sessions_list_filtered",
        rpc.request(
            "sessions.list",
            json!({"days":30,"tag_id":s.tag,"q":"ĀYAT","minutes_min":10,"minutes_max":30,
                   "values":{"focus":{"min":4,"max":5}}}),
        ),
    );
    assert_eq!(filtered["rows"].as_array().unwrap().len(), 1);
    assert_eq!(filtered["rows"][0]["note"], "Āyat 1–10 of al-Mulk");

    let everything = good(
        &mut rpc,
        "sessions.list",
        json!({"days":30,"with_note":false}),
    );
    assert_eq!(everything["rows"].as_array().unwrap().len(), 5);
    let gym = good(
        &mut rpc,
        "sessions.list",
        json!({"days":30,"with_note":false,"project_id":s.gym,"values":{"focus":{"max":2}}}),
    );
    assert_eq!(gym["rows"].as_array().unwrap().len(), 1);
    let none = good(
        &mut rpc,
        "sessions.list",
        json!({"days":30,"project_id":"none","item_id":s.walk}),
    );
    assert_eq!(titles(&none), [json!("Walk")]);

    let mut seen = Vec::new();
    let mut params = json!({"days":30,"with_note":false,"limit":2});
    loop {
        let page = good(&mut rpc, "sessions.list", params.clone());
        seen.extend(
            page["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["id"].clone()),
        );
        if page["next"].is_null() {
            break;
        }
        params["before"] = page["next"].clone();
    }
    let all: Vec<Value> = everything["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].clone())
        .collect();
    assert_eq!(seen, all);

    for (params, field) in [
        (json!({"limit":0}), "limit"),
        (json!({"limit":501}), "limit"),
        (json!({"minutes_min":20,"minutes_max":10}), "minutes_max"),
        (json!({"values":{"focus":{"min":5,"max":1}}}), "values"),
        (json!({"before":"garbage"}), "before"),
    ] {
        bad(&mut rpc, "sessions.list", params, "validation", Some(field));
    }
    bad(
        &mut rpc,
        "sessions.list",
        json!({"values":{"unknown":{"min":1}}}),
        "not_found",
        None,
    );
    bad(
        &mut rpc,
        "sessions.list",
        json!({"item_id":"missing"}),
        "not_found",
        None,
    );
}

fn totals(result: &Value) -> (i64, i64) {
    (
        result["totals"]["minutes"].as_i64().unwrap(),
        result["totals"]["sessions"].as_i64().unwrap(),
    )
}

#[test]
fn stats_series_draws_day_lines_and_splits_by_task() {
    let h = Harness::new();
    let s = seed(&h);
    let mut rpc = Session::open(&h, NOW);
    let mut fixtures = Fixtures::default();
    let range = json!({"from":"2031-05-08","to":"2031-05-14"});
    let mut params = range.clone();
    params["measures"] = json!(["minutes", "focus", "distractions_per_hour"]);
    let day = fixtures.check("stats_series_day", rpc.request("stats.series", params));
    assert_eq!(day["interval"], "day");
    assert_eq!(day["periods"].as_array().unwrap().len(), 7);
    let series = day["series"].as_array().unwrap();
    assert_eq!(series.len(), 3);
    // 8 May holds Read 30 min focus 4; 9 May has no session: a gap, 0 minutes.
    assert_eq!(series[0]["points"][0]["value"], 30.0);
    assert_eq!(series[0]["points"][1]["value"], 0.0);
    assert_eq!(series[1]["points"][0]["value"], 4.0);
    assert_eq!(series[1]["points"][1]["value"], Value::Null);
    assert_eq!(series[1]["scale_max"], 5);

    let mut params = range.clone();
    params["measures"] = json!(["minutes"]);
    params["split"] = json!("item");
    params["interval"] = json!("week");
    let split = fixtures.check(
        "stats_series_split_item",
        rpc.request("stats.series", params),
    );
    let keys: Vec<&str> = split["series"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, [s.read.as_str(), "none", s.walk.as_str()]);
    assert_eq!(split["series"][0]["label"], "Read");
    assert_eq!(split["periods"], json!(["2031-05-08", "2031-05-12"]));
    bad(
        &mut rpc,
        "stats.series",
        range.clone(),
        "validation",
        Some("measures"),
    );
    let mut params = range;
    params["measures"] = json!(["nope"]);
    bad(&mut rpc, "stats.series", params, "not_found", None);
}

#[test]
fn stats_get_and_table_narrow_sessions_by_duration_and_metric_values() {
    let h = Harness::new();
    let s = seed(&h);
    let mut rpc = Session::open(&h, NOW);
    let mut fixtures = Fixtures::with_ids(&[&s.tag, &s.quran]);
    let range = json!({"from":"2031-05-01","to":"2031-05-14"});
    let with = |extra: Value| {
        let mut params = range.clone();
        for (key, value) in extra.as_object().unwrap() {
            params[key] = value.clone();
        }
        params
    };
    let everything = good(&mut rpc, "stats.get", range.clone());
    assert_eq!(totals(&everything), (90, 5));
    assert_eq!(everything["filter"]["minutes_min"], Value::Null);
    assert_eq!(everything["filter"]["values"], json!({}));

    // Tag Study, 10+ minutes, focus 3 or more: the read log (4) and the timer (3).
    let narrowed = fixtures.check(
        "stats_metric_filter",
        rpc.request(
            "stats.get",
            with(json!({"tag_id":s.tag,"minutes_min":10,"values":{"focus":{"min":3}}})),
        ),
    );
    assert_eq!(totals(&narrowed), (55, 2));
    assert_eq!(narrowed["filter"]["minutes_min"], 10);
    assert_eq!(
        narrowed["filter"]["values"]["focus"],
        json!({"min":3.0,"max":null})
    );
    // Plan and capacity do not depend on the session bounds.
    let plan = |stats: &Value| {
        stats["days"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| (d["planned_minutes"].clone(), d["capacity_minutes"].clone()))
            .collect::<Vec<_>>()
    };
    let tagged = good(&mut rpc, "stats.get", with(json!({"tag_id":s.tag})));
    assert_eq!(plan(&narrowed), plan(&tagged));

    let duration = good(&mut rpc, "stats.get", with(json!({"minutes_max":20})));
    // A tick has no minutes, so no duration bound matches it.
    assert_eq!(totals(&duration), (35, 2), "gym and walk");
    let by_project = good(
        &mut rpc,
        "stats.get",
        with(json!({"project_id":s.gym,"values":{"focus":{"max":2}}})),
    );
    assert_eq!(totals(&by_project), (20, 1));
    let unassigned = good(
        &mut rpc,
        "stats.get",
        with(json!({"project_id":"none","minutes_max":20})),
    );
    assert_eq!(totals(&unassigned), (15, 1));
    let no_item = good(
        &mut rpc,
        "stats.get",
        with(json!({"item_id":"none","values":{"focus":{"min":2}}})),
    );
    assert_eq!(totals(&no_item), (20, 1), "the gym log has no task");
    let no_tag = good(
        &mut rpc,
        "stats.get",
        with(json!({"tag_id":"none","values":{"distractions":{"min":1}}})),
    );
    assert_eq!(totals(&no_tag), (20, 1));
    let empty = good(
        &mut rpc,
        "stats.get",
        with(json!({"values":{"focus":{"min":6}}})),
    );
    assert_eq!(totals(&empty), (0, 0));
    assert_eq!(empty["projects"], json!([]));

    let table = fixtures.check(
        "stats_table_metric_filter",
        rpc.request(
            "stats.table",
            with(json!({"group":"project","tag_id":s.tag,"minutes_min":10,
                        "values":{"focus":{"min":3,"max":4}}})),
        ),
    );
    assert_eq!(keys(&table), [s.quran.as_str()]);
    assert_eq!(totals(&table), (55, 2));
    assert_eq!(
        table["filter"]["values"]["focus"],
        json!({"min":3.0,"max":4.0})
    );
    for (extra, expected, group) in [
        (json!({"minutes_max":20}), (35, 2), "week"),
        (
            json!({"project_id":"none","minutes_max":20}),
            (15, 1),
            "item",
        ),
        (
            json!({"item_id":"none","values":{"focus":{"min":2}}}),
            (20, 1),
            "day",
        ),
        (
            json!({"tag_id":"none","values":{"distractions":{"min":1}}}),
            (20, 1),
            "project",
        ),
        (json!({"values":{"focus":{"min":6}}}), (0, 0), "tag"),
    ] {
        let mut params = with(extra);
        params["group"] = json!(group);
        let result = good(&mut rpc, "stats.table", params);
        assert_eq!(totals(&result), expected, "{group}");
    }

    for method in ["stats.get", "stats.table"] {
        for (params, field) in [
            (json!({"minutes_min":20,"minutes_max":10}), "minutes_max"),
            (json!({"minutes_min":-1}), "minutes_min"),
            (json!({"values":{"focus":{"min":5,"max":1}}}), "values"),
        ] {
            bad(&mut rpc, method, with(params), "validation", Some(field));
        }
        bad(
            &mut rpc,
            method,
            with(json!({"values":{"unknown":{"min":1}}})),
            "not_found",
            None,
        );
    }
}
