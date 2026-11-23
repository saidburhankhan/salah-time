#!/usr/bin/env node
"use strict"

// Plain node test for the widget's pure session rules (SessionLogic.js).
// Fixture shapes come from crates/salah-cli/tests/fixtures/rpc/.

const fs = require("fs")
const path = require("path")
const vm = require("vm")

const helperPath = path.join(__dirname, "..", "SessionLogic.js")
let source = fs.readFileSync(helperPath, "utf8")
source = source.replace(/^\.pragma library\s*/, "")
const logic = {}
vm.createContext(logic)
vm.runInContext(source, logic, { filename: helperPath })

const fixtures = path.join(__dirname, "..", "..", "..", "crates", "salah-cli", "tests", "fixtures", "rpc")
function fixture(name) {
  return JSON.parse(fs.readFileSync(path.join(fixtures, name), "utf8")).result
}

let checks = 0
function assert(condition, message) {
  checks++
  if (!condition) throw new Error("FAIL: " + message)
  console.log("ok " + checks + " - " + message)
}
function same(actual, expected, message) {
  const ok = JSON.stringify(actual) === JSON.stringify(expected)
  assert(ok, ok ? message : message + " (got " + JSON.stringify(actual) + ")")
}
function ids(rows) { return rows.map(function(row) { return row.metric_id }) }

const day = fixture("day_tracking.json")
const status = fixture("status_running.json")
const projects = fixture("projects_list.json")
const catalogue = fixture("metrics_list.json")
const quran = day.blocks[1].items[0]
const walk = day.blocks[1].items[1]

// ---- done / untick --------------------------------------------------------

same(logic.doneAction({ "sessions": [] }), { "action": "tick", "ids": [] },
  "an item without sessions today is ticked")
same(logic.doneAction(walk), { "action": "untick", "ids": ["<id:8>"] },
  "an item whose only session is a tick is unticked")
same(logic.doneAction({ "sessions": [walk.sessions[0], { "id": "t2", "minutes": null, "running": false }] }),
  { "action": "untick", "ids": ["<id:8>", "t2"] },
  "untick removes every tick, never logs a second one")
same(logic.doneAction(quran).action, "timed", "an item with timed sessions is not toggled")
same(logic.doneAction(status.planner.items[0]).action, "timed",
  "a running session counts as timed, not as a tick")
same(logic.doneAction({ "sessions": [walk.sessions[0], quran.sessions[0]] }).action, "timed",
  "a tick next to a timed session does not untick")
assert(logic.isTick(walk.sessions[0]) && !logic.isTick(quran.sessions[0])
  && !logic.isTick(status.running), "isTick: no minutes and not running")

// ---- metric ordering ----------------------------------------------------

same(ids(logic.orderMetrics([
  { "metric_id": "a", "required": false },
  { "metric_id": "b", "required": true },
  { "metric_id": "c", "required": false },
  { "metric_id": "d", "required": true }
])), ["b", "d", "a", "c"], "required metrics come first, each group keeps its order")

same(ids(logic.stopMetrics(status.prompt, status.running, catalogue)),
  ["focus", "quality", "<id:1>"], "stop uses status.prompt in required-first order")
same(ids(logic.stopMetrics([status.prompt[2], status.prompt[0]], status.running, catalogue)),
  ["focus", "<id:1>"], "stop reorders an optional-first prompt")
const projectless = logic.stopMetrics([], { "id": "s", "project_id": null, "running": true }, catalogue)
same(ids(projectless), ["focus", "quality"], "a session without a project asks the built-ins")
assert(projectless.every(function(row) { return row.required === false && row.kind === "scale" && row.scale_max === 5 }),
  "the built-ins are optional 1..5 scales")
same(ids(logic.stopMetrics([], { "id": "s", "project_id": "p" }, catalogue)), [],
  "a project without metrics asks nothing")
same(ids(logic.stopMetrics([], { "id": "s", "project_id": null }, [])), ["focus", "quality"],
  "the built-ins have a fallback before metrics.list loads")
same(ids(logic.builtinMetrics([{ "id": "focus", "name": "Focus", "kind": "scale", "scale_max": 5, "archived": true }])),
  ["quality"], "an archived built-in is not asked")

const projectRows = logic.projectMetrics(projects[0])
same(ids(projectRows), ["focus", "quality", "<id:1>"], "project metrics follow position, required first")
same(projectRows[2], { "metric_id": "<id:1>", "name": "Pages read", "unit": "pages", "kind": "count",
  "scale_max": null, "required": false }, "a count metric keeps its unit")
const shuffled = JSON.parse(JSON.stringify(projects[0]))
shuffled.metrics[0].required = false
shuffled.metrics[2].required = true
shuffled.metrics[1].metric.archived = true
same(ids(logic.projectMetrics(shuffled)), ["<id:1>", "focus"],
  "a required custom metric moves first and an archived one is left out")

// ---- log targets ----------------------------------------------------------

const targets = logic.logTargets(status.planner.items, projects)
same(targets.map(function(t) { return t.kind + ":" + t.label }),
  ["item:Read Quran", "item:Walk", "project:Quran"], "today's items, then projects")
same(targets[0].projectId, "<id:3>", "an item target carries its project")
same(logic.logTargets([], [{ "id": "x", "name": "Old", "archived": true }]), [],
  "archived projects are not offered")
same(ids(logic.logMetrics(targets[0], projects, catalogue)), ["focus", "quality", "<id:1>"],
  "logging against an item asks its project's metrics")
same(ids(logic.logMetrics(targets[1], projects, catalogue)), ["focus", "quality"],
  "logging against an item without a project asks the built-ins")
assert(logic.logMetrics(targets[1], projects, catalogue).every(function(row) { return !row.required }),
  "and they are optional")
same(ids(logic.logMetrics(targets[2], projects, catalogue)), ["focus", "quality", "<id:1>"],
  "logging against a project asks its metrics")
same(logic.logMetrics(null, projects, catalogue), [], "no target asks nothing")

// ---- values ---------------------------------------------------------------

const rows = logic.stopMetrics(status.prompt, status.running, catalogue)
same(logic.missingRequired(rows, {}), ["focus", "quality"], "both required scales are missing")
same(logic.missingRequired(rows, { "focus": 4 }), ["quality"], "one filled")
same(logic.missingRequired(rows, { "focus": 4, "quality": 5 }), [], "all required filled")
same(logic.collectValues(rows, { "focus": 4, "quality": 5, "<id:1>": "6", "other": 1 }),
  { "focus": 4, "quality": 5, "<id:1>": 6 }, "values are numbers for the asked metrics only")
same(logic.collectValues(rows, { "focus": 4, "quality": 5, "<id:1>": "" }),
  { "focus": 4, "quality": 5 }, "an empty optional count is left out")
const check = [{ "metric_id": "fasted", "kind": "check", "required": true },
  { "metric_id": "walked", "kind": "check", "required": false }]
same(logic.missingRequired(check, {}), [], "a required check is never missing")
same(logic.collectValues(check, {}), { "fasted": 0 }, "an untouched required check is sent as 0")
same(logic.collectValues(check, { "walked": 1 }), { "fasted": 0, "walked": 1 }, "a switched check is sent")

// ---- parsing ----------------------------------------------------------------

same([logic.parseMinutes("15"), logic.parseMinutes(" 60 "), logic.parseMinutes("1440")], [15, 60, 1440],
  "minutes parse")
same([logic.parseMinutes(""), logic.parseMinutes("0"), logic.parseMinutes("1441"), logic.parseMinutes("-5"),
  logic.parseMinutes("1.5"), logic.parseMinutes("abc")], [null, null, null, null, null, null],
  "invalid minutes are rejected")
same([logic.parseCount("6"), logic.parseCount("2.5"), logic.parseCount("0")], [6, 2.5, 0], "counts parse")
same([logic.parseCount(""), logic.parseCount("-1"), logic.parseCount("x")], [null, null, null],
  "invalid counts are rejected")
same(logic.elapsedMinutes("2031-05-14T04:30:00Z", Date.parse("2031-05-14T04:42:59Z")), 12,
  "elapsed minutes floor")
same(logic.elapsedMinutes("", Date.now()), 0, "no start, no elapsed time")

console.log("1.." + checks)
