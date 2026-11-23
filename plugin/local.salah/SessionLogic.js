.pragma library

// Pure session rules for the bar widget: what tapping "done" does, which
// metrics a stop or log asks for and in what order, and which values a save
// sends. No I/O and no prayer-time maths; tests/session_logic_test.js covers it.

// Built-in metrics have fixed ids (interface.md). They are asked, optional,
// for a session without a project. The catalogue from metrics.list wins; this
// is only the shape used before it has loaded.
var BUILTIN_IDS = ["focus", "quality"]
var BUILTIN_FALLBACK = [
  { "metric_id": "focus", "name": "Focus", "unit": "", "kind": "scale", "scale_max": 5, "required": false },
  { "metric_id": "quality", "name": "Quality", "unit": "", "kind": "scale", "scale_max": 5, "required": false }
]

var MAX_MINUTES = 1440

function text(value) {
  return value === undefined || value === null ? "" : String(value)
}

// A session without minutes that is not running is a tick.
function isTick(session) {
  if (!session) return false
  var minutes = session.minutes
  return (minutes === undefined || minutes === null) && session.running !== true
}

// What a tap on an item's done box does (plan.md, Done/untick):
//   no sessions today            -> "tick": log one session without minutes
//   only ticks today             -> "untick": remove those ticks
//   any timed or running session -> "timed": nothing to toggle; the sessions
//                                   are adjusted in the item's session list
function doneAction(item) {
  var sessions = item && item.sessions ? item.sessions : []
  if (sessions.length === 0) return { "action": "tick", "ids": [] }
  var ids = []
  for (var i = 0; i < sessions.length; i++) {
    if (!isTick(sessions[i])) return { "action": "timed", "ids": [] }
    ids.push(text(sessions[i].id))
  }
  return { "action": "untick", "ids": ids }
}

// Required metrics first, then optional ones, each group in the order given
// (the project's metric position).
function orderMetrics(rows) {
  var required = []
  var optional = []
  var list = rows || []
  for (var i = 0; i < list.length; i++) {
    if (!list[i]) continue
    if (list[i].required === true) required.push(list[i])
    else optional.push(list[i])
  }
  return required.concat(optional)
}

function promptRow(metric, required) {
  var scaleMax = metric.scale_max
  return {
    "metric_id": text(metric.metric_id || metric.id),
    "name": text(metric.name),
    "unit": text(metric.unit),
    "kind": text(metric.kind),
    "scale_max": scaleMax === undefined || scaleMax === null ? null : Number(scaleMax),
    "required": required === true
  }
}

// The built-in focus and quality metrics as optional prompts.
function builtinMetrics(catalogue) {
  var rows = []
  var list = catalogue || []
  for (var b = 0; b < BUILTIN_IDS.length; b++) {
    var found = null
    for (var i = 0; i < list.length; i++)
      if (list[i] && text(list[i].id) === BUILTIN_IDS[b]) found = list[i]
    if (found && found.archived === true) continue
    rows.push(found ? promptRow(found, false) : BUILTIN_FALLBACK[b])
  }
  return rows
}

function findProject(projects, id) {
  var key = text(id)
  if (key === "") return null
  var list = projects || []
  for (var i = 0; i < list.length; i++)
    if (list[i] && text(list[i].id) === key) return list[i]
  return null
}

// A projects.list row's metric bindings as prompts, archived metrics left out.
function projectMetrics(project) {
  var bindings = (project && project.metrics ? project.metrics : []).slice()
  bindings.sort(function(a, b) { return (Number(a.position) || 0) - (Number(b.position) || 0) })
  var rows = []
  for (var i = 0; i < bindings.length; i++) {
    var metric = bindings[i].metric || {}
    if (metric.archived === true) continue
    var row = promptRow(metric, bindings[i].required === true)
    if (row.metric_id === "") row.metric_id = text(bindings[i].metric_id)
    rows.push(row)
  }
  return orderMetrics(rows)
}

// Metrics for stopping the running session. status.get's `prompt` is
// authoritative for a session with a project; a session without one still
// asks the built-ins, optionally.
function stopMetrics(prompt, running, catalogue) {
  var rows = prompt || []
  if (rows.length > 0) return orderMetrics(rows.map(function(row) {
    return promptRow(row, row.required === true)
  }))
  if (running && text(running.project_id) === "") return builtinMetrics(catalogue)
  return []
}

// Metrics for logging against a target: { kind: "item"|"project", id, projectId }.
function logMetrics(target, projects, catalogue) {
  if (!target || text(target.id) === "") return []
  var projectId = target.kind === "project" ? text(target.id) : text(target.projectId)
  if (projectId === "") return builtinMetrics(catalogue)
  var project = findProject(projects, projectId)
  if (!project) return builtinMetrics(catalogue)
  return projectMetrics(project)
}

function hasValue(values, id) {
  if (!values) return false
  var value = values[id]
  return value !== undefined && value !== null && value !== "" && !isNaN(Number(value))
}

// Required metrics still without a value. A check shows as "No" until it is
// switched on, so it always has a value.
function missingRequired(metrics, values) {
  var missing = []
  var list = metrics || []
  for (var i = 0; i < list.length; i++) {
    var metric = list[i]
    if (metric.required !== true || text(metric.kind) === "check") continue
    if (!hasValue(values, text(metric.metric_id))) missing.push(text(metric.metric_id))
  }
  return missing
}

// The `values` object a stop or log sends: only the asked metrics, only the
// filled ones, plus a required check that was left off as 0.
function collectValues(metrics, values) {
  var out = {}
  var list = metrics || []
  for (var i = 0; i < list.length; i++) {
    var id = text(list[i].metric_id)
    if (hasValue(values, id)) out[id] = Number(values[id])
    else if (list[i].required === true && text(list[i].kind) === "check") out[id] = 0
  }
  return out
}

// Whole minutes from a field: 1..1440, otherwise null.
function parseMinutes(value) {
  var raw = text(value).trim()
  if (!/^[0-9]+$/.test(raw)) return null
  var minutes = Number(raw)
  return minutes >= 1 && minutes <= MAX_MINUTES ? minutes : null
}

// A count field: a non-negative number, otherwise null.
function parseCount(value) {
  var raw = text(value).trim()
  if (!/^[0-9]+([.][0-9]+)?$/.test(raw)) return null
  return Number(raw)
}

// The things "+ Log" can be recorded against: today's items, then the
// active projects.
function logTargets(items, projects) {
  var targets = []
  var itemList = items || []
  for (var i = 0; i < itemList.length; i++) {
    var item = itemList[i]
    if (!item || text(item.id) === "") continue
    targets.push({
      "kind": "item",
      "id": text(item.id),
      "label": text(item.title),
      "projectId": item.project ? text(item.project.id) : "",
      "color": item.project ? text(item.project.color) : ""
    })
  }
  var projectList = projects || []
  for (var p = 0; p < projectList.length; p++) {
    var project = projectList[p]
    if (!project || project.archived === true || text(project.id) === "") continue
    targets.push({
      "kind": "project",
      "id": text(project.id),
      "label": text(project.name),
      "projectId": text(project.id),
      "color": text(project.color)
    })
  }
  return targets
}

// Whole minutes a running session has been going, from its started_at.
function elapsedMinutes(startedAt, nowMs) {
  var started = Date.parse(text(startedAt))
  if (isNaN(started)) return 0
  return Math.max(0, Math.floor((Number(nowMs) - started) / 60000))
}
