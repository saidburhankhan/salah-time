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
    lines.extend(rows(&changed["goals"]).iter().map(goal_line));
    for review in rows(&changed["reviews"]) {
        lines.push(format!("Saved review for {}", text(&review["week"])));
    }
    for item in rows(&changed["items"]) {
        let when = if item["backlog"].as_bool() == Some(true) {
            "backlog".to_owned()
        } else {
            format!(
                "{} after {}",
                text(&item["schedule_text"]),
                text(&item["block"])
            )
        };
        lines.push(format!(
            "item {}  {}  ({when})",
            short(&item["id"]),
            text(&item["title"]),
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
    lines.extend(rows(&changed["resources"]).iter().map(resource_line));
    for (kind, list) in [
        ("project", &changed["projects"]),
        ("epic", &changed["epics"]),
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

pub fn goals(result: &Value) -> String {
    rows(result)
        .iter()
        .map(goal_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn goal_line(row: &Value) -> String {
    let scope = row["scope_label"].as_str().unwrap_or("All work");
    let metric = row["metric_id"].as_str().unwrap_or("minutes");
    let direction = if row["direction"] == "at_most" {
        "at most"
    } else {
        "at least"
    };
    let value = row["progress"]["value"]
        .as_f64()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "—".into());
    format!(
        "{}  {}  {}  {} {} {}  ({} so far)",
        text(&row["id"]),
        scope,
        text(&row["period"]),
        direction,
        row["target"],
        metric,
        value
    )
}

pub fn goal_history(result: &Value) -> String {
    rows(result)
        .iter()
        .map(|p| {
            format!(
                "{} to {}  target {}  value {}  {}",
                text(&p["from"]),
                text(&p["to"]),
                p["target"],
                p["value"],
                if p["met"] == true { "met" } else { "not met" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn rate(value: &Value) -> String {
    value
        .as_f64()
        .map(|v| format!("{v:.1}/h"))
        .unwrap_or_else(|| "—".into())
}

fn session_line(row: &Value) -> String {
    let title = row["item"]["title"].as_str().unwrap_or("No task");
    let mut line = format!(
        "  {}  {}  {} min  {}",
        text(&row["day"]),
        text(&row["block"]),
        row["minutes"].as_i64().unwrap_or(0),
        title
    );
    if let Some(note) = row["note"].as_str().filter(|n| !n.is_empty()) {
        line.push_str(&format!("  \"{note}\""));
    }
    line
}

fn minutes_lines(title: &str, groups: &Value, out: &mut Vec<String>) {
    if rows(groups).is_empty() {
        return;
    }
    out.push(format!("{title}:"));
    for g in rows(groups) {
        out.push(format!("  {}  {} min", text(&g["name"]), g["minutes"]));
    }
}

/// The weekly review: summary lines, then the saved reflection.
pub fn review(result: &Value) -> String {
    let s = &result["summary"];
    let mut out = vec![format!("Week of {}", text(&result["week"]))];
    minutes_lines("By project", &s["by_project"], &mut out);
    minutes_lines("By epic", &s["by_epic"], &mut out);
    if !rows(&s["goals_met"]).is_empty() {
        out.push("Goals met:".into());
        out.extend(
            rows(&s["goals_met"])
                .iter()
                .map(|g| format!("  {}", goal_line(g))),
        );
    }
    out.push(format!(
        "Distractions: {} (previous week {})",
        rate(&s["distractions_per_hour"]),
        rate(&s["previous_distractions_per_hour"])
    ));
    out.push(format!("Open backlog tasks: {}", s["open_backlog_count"]));
    if !rows(&s["sessions_with_notes"]).is_empty() {
        out.push("Sessions with notes:".into());
        out.extend(rows(&s["sessions_with_notes"]).iter().map(session_line));
    }
    match result["note"].as_str().filter(|n| !n.is_empty()) {
        Some(note) => out.push(format!("Reflection:\n{note}")),
        None => out.push("No reflection saved.".into()),
    }
    out.join("\n")
}

/// Insights: minutes by block, weekday and hour, plus ranked sessions.
pub fn insights(result: &Value) -> String {
    let mut out = vec!["By block:".to_owned()];
    for b in rows(&result["by_block"]) {
        out.push(format!(
            "  {}  {} min  {} sessions  focus {}  distractions {}",
            text(&b["block"]),
            b["minutes"],
            b["sessions"],
            b["focus_avg"]
                .as_f64()
                .map_or("—".into(), |v| format!("{v:.1}")),
            rate(&b["distractions_per_hour"])
        ));
    }
    const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    out.push("By weekday:".into());
    for w in rows(&result["by_weekday"]) {
        let day = w["weekday"]
            .as_u64()
            .and_then(|d| DAYS.get((d as usize + 6) % 7).copied())
            .unwrap_or("?");
        out.push(format!(
            "  {day}  {} min  {} sessions  distractions {}",
            w["minutes"],
            w["sessions"],
            rate(&w["distractions_per_hour"])
        ));
    }
    out.push("By start hour:".into());
    for h in rows(&result["by_hour"])
        .iter()
        .filter(|h| h["sessions"] != 0)
    {
        out.push(format!(
            "  {:02}:00  {} min  {} sessions",
            h["hour"].as_u64().unwrap_or(0),
            h["minutes"],
            h["sessions"]
        ));
    }
    out.push("Distractions by week:".into());
    for w in rows(&result["distractions_by_week"]) {
        out.push(format!("  {}  {}", text(&w["week"]), rate(&w["per_hour"])));
    }
    out.push("Planned vs actual:".into());
    for r in rows(&result["on_plan"]) {
        out.push(format!(
            "  {}  {} of {} min on plan ({})",
            text(&r["name"]),
            r["on_plan_minutes"],
            r["minutes"],
            text(&r["planned_block"])
        ));
    }
    for (title, key) in [("Best sessions", "best"), ("Hardest sessions", "hardest")] {
        out.push(format!("{title}:"));
        out.extend(rows(&result[key]).iter().map(session_line));
    }
    out.join("\n")
}

pub fn resources(result: &Value) -> String {
    rows(result)
        .iter()
        .map(resource_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn resource_line(resource: &Value) -> String {
    let kind = text(&resource["kind"]);
    let mut line = format!("{kind} {}", text(&resource["id"]));
    for field in ["title", "body", "url", "remind_at"] {
        if let Some(value) = resource[field].as_str() {
            line.push_str("  ");
            line.push_str(value);
        }
    }
    if resource["done"] == true {
        line.push_str("  (done)");
    }
    line
}

pub fn items(result: &Value) -> String {
    rows(result)
        .iter()
        .map(|item| {
            format!(
                "{}  {:<8} {}  ({})",
                short(&item["id"]),
                item["block"].as_str().unwrap_or("backlog"),
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
            let metrics: Vec<&str> = rows(&project["metrics"])
                .iter()
                .map(|metric| text(&metric["metric_id"]))
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

/// Backlog rows in order, with the project and planned minutes.
pub fn backlog(result: &Value) -> String {
    let lines: Vec<String> = rows(result)
        .iter()
        .map(|item| {
            let minutes = match &item["planned_minutes"] {
                Value::Number(n) => format!("  {n} min"),
                _ => String::new(),
            };
            format!(
                "{}  {}{}{minutes}",
                short(&item["id"]),
                text(&item["title"]),
                item["project"]["name"]
                    .as_str()
                    .map(|name| format!("  [{name}]"))
                    .unwrap_or_default(),
            )
        })
        .collect();
    if lines.is_empty() {
        "the backlog is empty".into()
    } else {
        lines.join("\n")
    }
}

/// Epics with their status and recorded work.
pub fn epics(result: &Value) -> String {
    rows(result)
        .iter()
        .map(|epic| {
            format!(
                "{}  {:<8} {}  {} min in {} sessions",
                short(&epic["id"]),
                text(&epic["status"]),
                text(&epic["name"]),
                epic["total_minutes"],
                epic["sessions"],
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
