.pragma library

// Pure session rules shared by the widget and its Node checks.

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
//   otherwise                 -> "done": open the done panel
// Timed sessions alone never make an item done.
function doneAction(item) {
  var ids = tickIds(item)
  return { "action": ids.length > 0 ? "untick" : "done", "ids": ids }
}

function canEditMinutes(session) {
  return !!session && session.running !== true && session.minutes != null
}

function canAnnotateRunning(session) {
  return !!session && session.stale !== true
}

function noteTarget(kind, row, entry) {
  if (!row || !row.id) return null
  var version = row.version
  switch (kind) {
  case "main":
    return { method: "session.edit", field: "note", original: text(row.note), limit: 500,
      params: { id: row.id, expected_version: version } }
  case "entry":
    if (!entry || !entry.id || !Number.isInteger(version)) return null
    return { method: "session.note_edit", field: "text", original: text(entry.text), limit: 500,
      params: { session_id: row.id, note_id: entry.id, expected_version: version } }
  case "append":
    if (!canAnnotateRunning(row)) return null
    // Appends can coexist with a distraction bump, including the bar's
    // right-click bump immediately before its note editor opens.
    return { method: "session.note", field: "text", original: "", limit: 500, params: { id: row.id } }
  case "resource":
    return { method: "resource.edit", field: "body", original: text(row.body), limit: 5000,
      params: { id: row.id, expected_version: version } }
  default:
    return null
  }
}

function noteAttempt(previous, target, draft, newOpId) {
  var trimmed = text(draft).trim()
  if (!target || !trimmed || Array.from(trimmed).length > target.limit) return null
  var params = Object.assign({}, target.params)
  params[target.field] = trimmed
  var key = JSON.stringify([target.method, params])
  if (previous && previous.key === key) return previous
  return { opId: newOpId(), method: target.method, params: params, key: key }
}

function logParams(itemId, projectId, minutes, values, note, block, epicId) {
  var params = { values: values || {} }
  if (itemId) params.item_id = String(itemId)
  if (projectId) params.project_id = String(projectId)
  if (epicId) params.epic_id = String(epicId)
  if (minutes != null) params.minutes = Number(minutes)
  if (note) params.note = String(note)
  if (block) params.block = String(block)
  return params
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

// Item rows already carry their effective prompts, including an empty list.
function targetMetrics(target, projects, catalogue) {
  if (!target || text(target.id) === "") return []
  if (target.kind === "project") return projectMetrics(findProject(projects, target.id))
  if (target.metrics) return target.metrics
  // Pre-phase-2 daemons omit the field entirely and only support project defaults.
  var legacyProject = findProject(projects, target.projectId)
  return legacyProject ? projectMetrics(legacyProject) : builtinMetrics(catalogue)
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

function sessionMinuteCap(value) {
  return Number.isInteger(value) && value >= 1 && value <= MAX_MINUTES ? value : null
}

// Match the timer's rounding and use the server's Sleep cutoff when present.
function workedMinutes(startedAt, nowMs, maxMinutes) {
  var cap = sessionMinuteCap(maxMinutes) || MAX_MINUTES
  return Math.max(1, Math.min(cap, Math.round(elapsedSeconds(startedAt, nowMs) / 60)))
}

function subtractWorkedMinutes(minutes, cap, reduction) {
  return Math.max(1, Math.min(minutes, cap) - reduction)
}

function runningDisplaySeconds(session, nowMs) {
  if (session.stale !== true) return elapsedSeconds(session.started_at, nowMs)
  var cap = sessionMinuteCap(session.max_minutes)
  return cap === null ? null : cap * 60
}

// The server merges supplied values with counts recorded during the timer.
// Send only edits so a later bump on another surface survives a plain Save.
function pauseParams(session, metrics, edits, note, worked, initialMinutes, done) {
  var params = { "id": session.id }
  var values = collectValues(metrics, edits)
  if (Object.keys(values).length > 0) params.values = values
  var trimmed = text(note).trim()
  if (trimmed) params.note = trimmed
  var minutes = parseMinutes(worked)
  if (minutes !== null && minutes !== initialMinutes) params.minutes = minutes
  if (done && session.item_id) params.done = true
  return params
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
      "metrics": item.metrics,
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

// A running timer as minutes:seconds, never hours, because sessions are
// counted in minutes: "09:05", "47:30", "100:05". Two digits at least per
// field keep the text the same width while it ticks.
function clockText(totalSeconds) {
  var total = Math.max(0, Math.floor(Number(totalSeconds) || 0))
  function pad(value) { return value < 10 ? "0" + value : String(value) }
  return pad(Math.floor(total / 60)) + ":" + pad(total % 60)
}

function noteTime(value) {
  var at = new Date(value)
  return !value || isNaN(at.getTime()) ? "" : ("0" + at.getHours()).slice(-2)
    + ":" + ("0" + at.getMinutes()).slice(-2)
}
