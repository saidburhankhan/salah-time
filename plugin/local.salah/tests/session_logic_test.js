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

same(logic.doneAction({ "sessions": [] }), { "action": "done", "ids": [] },
  "an item without sessions opens the done panel")
same(logic.doneAction(walk), { "action": "untick", "ids": ["<id:8>"] },
  "an item with a tick is unticked")
same(logic.doneAction({ "sessions": [walk.sessions[0], { "id": "t2", "minutes": null, "running": false }] }).ids,
  ["<id:8>", "t2"], "untick removes every tick, never logs a second one")
same(logic.doneAction(quran), { "action": "done", "ids": [] },
  "timed sessions alone do not make an item done")
same(logic.doneAction({ "sessions": [walk.sessions[0], quran.sessions[0]] }),
  { "action": "untick", "ids": ["<id:8>"] }, "untick keeps the timed sessions")
same(logic.doneAction(status.planner.items[0]), { "action": "done", "ids": [] },
  "a running timer does not change the done action (the server stops it)")
assert(logic.isTick(walk.sessions[0]) && !logic.isTick(quran.sessions[0])
  && !logic.isTick(status.running), "isTick: no minutes and not running")
same(logic.tickIds(status.planner.items[0]), [], "a running session is not a tick")

// ---- metrics ----------------------------------------------------------------

const projectRows = logic.projectMetrics(projects[0])
const expectedProjectIds = projects[0].metrics.slice().sort((a, b) => a.position - b.position)
  .filter(binding => !binding.metric.archived).map(binding => binding.metric_id)
same(ids(projectRows), expectedProjectIds, "project metrics follow position")
same(projectRows[2], { "metric_id": "<id:1>", "name": "Pages read", "unit": "pages", "kind": "count",
  "scale_max": null }, "a count metric keeps its unit and carries no required flag")
const shuffled = JSON.parse(JSON.stringify(projects[0]))
shuffled.metrics = shuffled.metrics.slice(0, 3)
shuffled.metrics[0].position = 5
shuffled.metrics[2].required = true
shuffled.metrics[1].metric.archived = true
same(ids(logic.projectMetrics(shuffled)), ["<id:1>", "focus"],
  "position decides the order, required does not, and an archived metric is left out")

const builtins = logic.builtinMetrics(catalogue)
same(ids(builtins), ["focus", "quality"], "the built-ins are focus and quality")
assert(builtins.every(function(row) { return row.kind === "scale" && row.scale_max === 5 && !("required" in row) }),
  "the built-ins are 1..5 scales")
same(ids(logic.builtinMetrics([])), ["focus", "quality"], "the built-ins have a fallback before metrics.list loads")
same(ids(logic.builtinMetrics([{ "id": "focus", "name": "Focus", "kind": "scale", "scale_max": 5, "archived": true }])),
  ["quality"], "an archived built-in is not asked")

// ---- targets ----------------------------------------------------------------

const itemRows = status.planner.items.map((row, i) => ({
  ...row, metrics: i === 0 ? [projectRows[2], projectRows[0]] : []
}))
const targets = logic.logTargets(itemRows, projects)
same(targets.map(function(t) { return t.kind + ":" + t.label }),
  ["item:Read Quran", "item:Walk", "project:Quran"], "today's items, then projects")
same(targets[0].projectId, "<id:3>", "an item target carries its project")
same(logic.logTargets([], [{ "id": "x", "name": "Old", "archived": true }]), [],
  "archived projects are not offered")
same(ids(logic.targetMetrics(targets[0], projects, catalogue)), ["<id:1>", "focus"],
  "item prompts use the exact row metrics and order")
same(ids(logic.targetMetrics(targets[1], projects, catalogue)), [],
  "an explicitly empty list stays empty on an item without a project")
same(logic.targetMetrics({ ...targets[0], metrics: [] }, projects, catalogue), [],
  "a project item can opt out of every metric")
same(ids(logic.targetMetrics({ kind: "item", id: "old" }, projects, catalogue)), ["focus", "quality"],
  "a legacy item without a project or prompts gets the built-in ratings")
same(ids(logic.targetMetrics({ kind: "item", id: "old", projectId: projects[0].id }, projects, catalogue)),
  expectedProjectIds, "a legacy server without per-item metrics keeps its project defaults")
same(ids(logic.targetMetrics(targets[2], projects, catalogue)), expectedProjectIds,
  "a project asks its metrics")
same(ids(logic.targetMetrics({ "kind": "project", "id": "p", "projectId": "p" },
  [{ "id": "p", "name": "Bare", "metrics": [] }], catalogue)), [], "a project without metrics asks nothing")
same(logic.targetMetrics(null, projects, catalogue), [], "no target asks nothing")

// ---- values ---------------------------------------------------------------

same(logic.collectValues(projectRows, {}), {}, "nothing filled sends no values (Skip)")
same(logic.collectValues(projectRows, { "focus": 4, "<id:1>": "6", "other": 1 }),
  { "focus": 4, "<id:1>": 6 }, "only the asked, filled metrics are sent, as numbers")
same(logic.collectValues(projectRows, { "focus": 4, "quality": null, "<id:1>": "" }),
  { "focus": 4 }, "cleared and empty values are left out")
const check = [{ "metric_id": "fasted", "kind": "check" }]
same(logic.collectValues(check, {}), {}, "an untouched check is not sent")
same(logic.collectValues(check, { "fasted": 0 }), { "fasted": 0 }, "a check switched off is sent as 0")
same(logic.collectValues(check, { "fasted": 1 }), { "fasted": 1 }, "a check switched on is sent as 1")

// ---- pause ----------------------------------------------------------------

const running = { id: "s", item_id: "i", values: { distractions: 3 } }
const prompts = [projectRows[0], { metric_id: "distractions", kind: "count" }]
const pause = (edits, note, minutes, done, session = running) =>
  logic.pauseParams(session, prompts, edits, note, minutes, 30, done)
same(pause({}, "  ", "30", false), { id: "s" },
  "empty Save leaves minutes and live counts to the server")
same(pause({ focus: "", distractions: null, other: 2 }, "", "", true), { id: "s", done: true },
  "empty Save & Done sends only the session id and done flag")
same(pause({ distractions: "0", focus: 4, quality: 5 }, "  paused  ", "25", false),
  { id: "s", values: { focus: 4, distractions: 0 }, note: "paused", minutes: 25 },
  "Save sends asked edits, including zero, trimmed note and changed minutes")
same(pause({ focus: 4 }, "ok", "15", true),
  { id: "s", values: { focus: 4 }, note: "ok", minutes: 15, done: true },
  "Save & Done uses one stop carrying the edits and done flag")
same(pause({}, "", "030", true, { id: "p", item_id: null }), { id: "p" },
  "project-only pause omits done and a numerically unchanged duration")
const start = "2031-05-14T04:30:00Z"
same([0, 89, 90, 999999].map(s => logic.workedMinutes(start, Date.parse(start) + 1000 * s)),
  [1, 1, 2, 1440], "Worked retains the stop rounding and day limit for older running reports")
const stale = fixture("status_stale.json").running
const staleStart = Date.parse(stale.started_at)
const afterSleep = staleStart + (stale.max_minutes + 600) * 60000
same(logic.workedMinutes(stale.started_at, afterSleep, stale.max_minutes), stale.max_minutes,
  "Worked uses the stale session's Sleep cutoff")
same(logic.workedMinutes(start, Date.parse(start) + 32 * 60000, 30), 30,
  "Worked never exceeds the last server cap for a running timer")
same(logic.workedMinutes(start, Date.parse(start) + 42 * 1000, 30), 1,
  "Worked starts at elapsed when the server cap is higher")
same([5, 15, 30].map(reduction => logic.subtractWorkedMinutes(4, 10, reduction)),
  [1, 1, 1], "all three correction chips stop at one minute")
same([5, 15, 30].map(reduction => logic.subtractWorkedMinutes(600, 120, reduction)),
  [115, 105, 90], "correction chips start at the cap if the field is above it")
same([afterSleep, afterSleep + 30000].map(now => logic.runningDisplaySeconds(stale, now)),
  [stale.max_minutes * 60, stale.max_minutes * 60], "the stale clock holds at the Sleep cutoff")
same(logic.runningDisplaySeconds({ started_at: start, stale: false, max_minutes: 1 },
  Date.parse(start) + 95000), 95, "an active clock continues counting between server reports")
same(logic.runningDisplaySeconds({ started_at: start, stale: true }, afterSleep), null,
  "a stale report without a cap has no displayable duration")

// ---- session notes ---------------------------------------------------------

const localTime = new Date(2031, 4, 14, 9, 5).toISOString()
same(logic.noteTime(localTime), "09:05", "timestamped entries display the local time")
same(logic.noteTime(null), "", "missing timestamps do not add a visible time")
same(logic.noteTarget("main", { id: "s", note: "<b>older</b>" }).original, "<b>older</b>",
  "existing main note text is literal")
same(logic.noteTarget("main", { id: "s" }).original, "", "missing main note gives no visible text")

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
same(logic.elapsedSeconds("2031-05-14T04:30:00Z", Date.parse("2031-05-14T04:39:05Z")), 545,
  "elapsed seconds count from started_at")
same(logic.clockText(545), "09:05", "under an hour: two-digit minutes and seconds")
same(logic.clockText(5), "00:05", "seconds alone still keep two digits per field")
same(logic.clockText(3723), "62:03", "no hours: minutes keep counting past 60")
same(logic.clockText(6005), "100:05", "past 99 minutes")
same(logic.clockText(-4), "00:00", "never negative")

// ---- annotate, minutes, note retry, log params ----------------------------
assert(logic.canEditMinutes({ "minutes": 15, "running": false }), "finished timed session is editable")
assert(!logic.canEditMinutes({ "minutes": null, "running": false }), "completion tick is not editable")
assert(!logic.canEditMinutes({ "minutes": 3, "running": true }), "running session is not editable")
assert(logic.canAnnotateRunning({ "id": "r" }) && !logic.canAnnotateRunning({ "id": "r", "stale": true })
  && !logic.canAnnotateRunning(null), "stale or missing running session cannot be annotated")
let n = 0
const mint = () => "op" + (++n)
const append = logic.noteTarget("append", { id: "s1", running: true, version: 1 })
const first = logic.noteAttempt(null, append, " hi ", mint)
same({ opId: first.opId, method: first.method, params: first.params },
  { opId: "op1", method: "session.note", params: { id: "s1", text: "hi" } },
  "running note append can coexist with a distraction bump")
assert(logic.noteAttempt(first, append, "hi  ", mint) === first, "unchanged retry keeps the op id")
assert(logic.noteAttempt(first, append, "hi there", mint).opId === "op2", "edited draft gets a new op id")
assert(logic.noteAttempt(first, logic.noteTarget("append", { id: "s2" }), "hi", mint).opId === "op3",
  "other session gets a new op id")
const addedNote = fixture("session_note_at.json").changed.sessions[0]
const revisedNote = fixture("session_note_edit.json").changed.sessions[0]
const entryTarget = logic.noteTarget("entry", addedNote, addedNote.notes[0])
const entryAttempt = logic.noteAttempt(null, entryTarget, revisedNote.notes[0].text, mint)
same(entryAttempt.method, "session.note_edit", "entry editor selects the versioned note RPC")
same(entryAttempt.params, {
  session_id: revisedNote.id, note_id: revisedNote.notes[0].id,
  expected_version: addedNote.version, text: revisedNote.notes[0].text
}, "entry edit uses the parent version from the opening snapshot")
assert(logic.noteAttempt(entryAttempt, entryTarget, revisedNote.notes[0].text, mint) === entryAttempt,
  "replaying a note edit keeps its version and operation ID")
const mainAttempt = logic.noteAttempt(null, logic.noteTarget("main", revisedNote), "One\nTwo", mint)
same([mainAttempt.method, mainAttempt.params],
  ["session.edit", { id: revisedNote.id, expected_version: revisedNote.version, note: "One\nTwo" }],
  "main note sends literal multiline text and the captured version")
const resource = logic.noteTarget("resource", { id: "resource", version: 2, body: "draft" })
same(logic.noteAttempt(null, resource, "Some text", mint).params,
  { id: "resource", expected_version: 2, body: "Some text" }, "resource note edit writes only its body")
same(logic.noteAttempt(null, entryTarget, "  \n  ", mint), null, "blank note cannot clear an entry")
same(logic.noteAttempt(null, entryTarget, "😀".repeat(501), mint), null, "note limit counts code points")
same(logic.logParams("i", "", null, null, "", "", ""), { "values": {}, "item_id": "i" }, "tick log has no minutes")
same(logic.logParams("", "p", 0, { "a": 1 }, "n", "isha", "e"),
  { "values": { "a": 1 }, "project_id": "p", "epic_id": "e", "minutes": 0, "note": "n", "block": "isha" }, "full log")

console.log("1.." + checks)
