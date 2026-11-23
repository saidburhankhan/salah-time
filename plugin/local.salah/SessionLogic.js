.pragma library

// Pure session rules for the bar widget: what tapping "done" does, which
// metrics the done and log panels ask, and which values a save sends. No I/O
// and no prayer-time maths; tests/session_logic_test.js covers it.
//
// User decisions (plan.md, 2026-11-23): a timer is play/pause and asks
// nothing; "done" is an explicit tick that may carry metric values; every
// metric is optional.

// Built-in metrics have fixed ids (interface.md). They are asked for an item
// or session without a project. The catalogue from metrics.list wins; this is
// only the shape used before it has loaded.
var BUILTIN_IDS = ["focus", "quality"]
var BUILTIN_FALLBACK = [
  { "metric_id": "focus", "name": "Focus", "unit": "", "kind": "scale", "scale_max": 5 },
  { "metric_id": "quality", "name": "Quality", "unit": "", "kind": "scale", "scale_max": 5 }
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

// The ids of an item's tick sessions today.
function tickIds(item) {
  var sessions = item && item.sessions ? item.sessions : []
  var ids = []
  for (var i = 0; i < sessions.length; i++)
    if (isTick(sessions[i])) ids.push(text(sessions[i].id))
  return ids
}

// What a tap on an item's done box does (plan.md, user decisions):
//   the item has a tick today -> "untick": remove its tick(s); timed sessions stay
//   otherwise                 -> "done": open the done panel; when the item's
//                                timer is running, `stopId` is stopped first
// Timed sessions alone never make an item done.
function doneAction(item) {
  var ids = tickIds(item)
  if (ids.length > 0) return { "action": "untick", "ids": ids, "stopId": "" }
  var sessions = item && item.sessions ? item.sessions : []
  var stopId = ""
  for (var i = 0; i < sessions.length; i++)
    if (sessions[i] && sessions[i].running === true) stopId = text(sessions[i].id)
  return { "action": "done", "ids": [], "stopId": stopId }
}

function promptRow(metric) {
  var scaleMax = metric.scale_max
  return {
    "metric_id": text(metric.metric_id || metric.id),
    "name": text(metric.name),
    "unit": text(metric.unit),
    "kind": text(metric.kind),
    "scale_max": scaleMax === undefined || scaleMax === null ? null : Number(scaleMax)
  }
}

// The built-in focus and quality metrics.
function builtinMetrics(catalogue) {
  var rows = []
  var list = catalogue || []
  for (var b = 0; b < BUILTIN_IDS.length; b++) {
    var found = null
    for (var i = 0; i < list.length; i++)
      if (list[i] && text(list[i].id) === BUILTIN_IDS[b]) found = list[i]
    if (found && found.archived === true) continue
    rows.push(found ? promptRow(found) : BUILTIN_FALLBACK[b])
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

// A projects.list row's metric bindings as prompts, in the project's order,
// archived metrics left out. `required` is not enforced and not shown.
function projectMetrics(project) {
  var bindings = (project && project.metrics ? project.metrics : []).slice()
  bindings.sort(function(a, b) { return (Number(a.position) || 0) - (Number(b.position) || 0) })
  var rows = []
  for (var i = 0; i < bindings.length; i++) {
    var metric = bindings[i].metric || {}
    if (metric.archived === true) continue
    var row = promptRow(metric)
    if (row.metric_id === "") row.metric_id = text(bindings[i].metric_id)
    rows.push(row)
  }
  return rows
}

// Metrics for a done tick or a log against a target
// ({ kind: "item"|"project", id, projectId }): the project's metrics, or the
// built-ins when there is no project.
function targetMetrics(target, projects, catalogue) {
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

// The `values` object a done or log sends: only the asked metrics, and only
// the ones that were filled (every metric is optional).
function collectValues(metrics, values) {
  var out = {}
  var list = metrics || []
  for (var i = 0; i < list.length; i++) {
    var id = text(list[i].metric_id)
    if (hasValue(values, id)) out[id] = Number(values[id])
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

// Whole seconds a running session has been going, from its started_at.
function elapsedSeconds(startedAt, nowMs) {
  var started = Date.parse(text(startedAt))
  if (isNaN(started)) return 0
  return Math.max(0, Math.floor((Number(nowMs) - started) / 1000))
}

// Whole minutes a running session has been going, from its started_at.
function elapsedMinutes(startedAt, nowMs) {
  return Math.floor(elapsedSeconds(startedAt, nowMs) / 60)
}

// A running timer as a clock with two digits per field, so the text keeps
// its width while it ticks: "09:05" under an hour, then "01:02:03".
function clockText(totalSeconds) {
  var total = Math.max(0, Math.floor(Number(totalSeconds) || 0))
  function pad(value) { return value < 10 ? "0" + value : String(value) }
  var hours = Math.floor(total / 3600)
  var clock = pad(Math.floor((total % 3600) / 60)) + ":" + pad(total % 60)
  return hours > 0 ? pad(hours) + ":" + clock : clock
}
