//! Short human renderings of protocol results. `--json` prints the result
//! itself; these are for people at a terminal.

use serde_json::Value;

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}

/// The first eight characters of an id, enough to type back as a prefix.
fn short(value: &Value) -> &str {
    let id = text(value);
    id.get(..8).unwrap_or(id)
}

fn rows(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or(&[])
}

pub fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

/// One line per entity a mutation wrote.
pub fn mutation(result: &Value) -> String {
    let changed = &result["changed"];
    let mut lines = Vec::new();
    for item in rows(&changed["items"]) {
        lines.push(format!(
            "item {}  {}  ({} after {})",
            short(&item["id"]),
            text(&item["title"]),
            text(&item["schedule_text"]),
            text(&item["block"]),
        ));
    }
    for session in rows(&changed["sessions"]) {
        let minutes = match (&session["minutes"], session["running"].as_bool()) {
            (_, Some(true)) => "running".to_owned(),
            (Value::Number(minutes), _) => format!("{minutes} min"),
            _ => "ticked".to_owned(),
        };
        lines.push(format!("session {}  {minutes}", short(&session["id"])));
    }
    for (kind, list) in [
        ("project", &changed["projects"]),
        ("tag", &changed["tags"]),
        ("metric", &changed["metrics"]),
    ] {
        for row in rows(list) {
            lines.push(format!(
                "{kind} {}  {}",
                short(&row["id"]),
                text(&row["name"])
            ));
        }
    }
    for mark in rows(&changed["marks"]) {
        let state = if mark["deleted_at"].is_null() {
            "marked"
        } else {
            "unmarked"
        };
        lines.push(format!(
            "{} {}  {state}",
            text(&mark["prayer"]),
            text(&mark["day"])
        ));
    }
    for id in rows(&changed["deleted"]) {
        lines.push(format!("removed {}", short(id)));
    }
    if lines.is_empty() {
        "nothing changed".into()
    } else {
        lines.join("\n")
    }
}

pub fn items(result: &Value) -> String {
    rows(result)
        .iter()
        .map(|item| {
            format!(
                "{}  {:<8} {}  ({})",
                short(&item["id"]),
                text(&item["block"]),
                text(&item["title"]),
                text(&item["schedule_text"]),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn projects(result: &Value) -> String {
    rows(result)
        .iter()
        .map(|project| {
            let metrics: Vec<String> = rows(&project["metrics"])
                .iter()
                .map(|m| {
                    let suffix = if m["required"].as_bool() == Some(true) {
                        ""
                    } else {
                        "?"
                    };
                    format!("{}{suffix}", text(&m["metric_id"]))
                })
                .collect();
            format!(
                "{}  {}  {}  [{}]",
                short(&project["id"]),
                text(&project["color"]),
                text(&project["name"]),
                metrics.join(", "),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Tags and metrics: id and name.
pub fn named_rows(result: &Value) -> String {
    rows(result)
        .iter()
        .map(|row| format!("{}  {}", short(&row["id"]), text(&row["name"])))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The day's blocks with their items, done ones ticked.
pub fn day(plan: &Value) -> String {
    let fulfilment = &plan["fulfilment"];
    let mut lines = vec![format!(
        "{}  {}/{} done · {}/{} min",
        text(&plan["day"]),
        fulfilment["done_items"],
        fulfilment["planned_items"],
        fulfilment["actual_minutes"],
        fulfilment["planned_minutes"],
    )];
    for block in rows(&plan["blocks"]) {
        lines.push(format!(
            "{}–{}  {} / {} min",
            text(&block["start"]),
            text(&block["end"]),
            block["actual_minutes"],
            block["capacity_minutes"],
        ));
        for item in rows(&block["items"]) {
            let mark = if item["done"].as_bool() == Some(true) {
                "x"
            } else {
                " "
            };
            lines.push(format!(
                "  [{mark}] {}  {}  {}",
                text(&item["title"]),
                text(&item["detail_text"]),
                short(&item["id"]),
            ));
        }
    }
    lines.join("\n")
}

pub fn stats(stats: &Value) -> String {
    let totals = &stats["totals"];
    let mut lines = vec![format!(
        "{} to {}: {} min in {} sessions on {} days",
        text(&stats["range"]["from"]),
        text(&stats["range"]["to"]),
        totals["minutes"],
        totals["sessions"],
        totals["active_days"],
    )];
    for project in rows(&stats["projects"]) {
        lines.push(format!(
            "  {}  {} min",
            text(&project["name"]),
            project["minutes"],
        ));
    }
    lines.join("\n")
}
