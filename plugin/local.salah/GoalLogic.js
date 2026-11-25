.pragma library

// Panel header goal chips from `status.get.goals`. Pure logic: QML passes the
// lookup lists and already translated strings, and renders the result.

var MAX_CHIPS = 3

function list(rows) {
  // QML-converted arrays fail Array.isArray, so accept any array-like.
  if (!rows || typeof rows.length !== "number" || typeof rows === "string") return []
  return Array.prototype.slice.call(rows)
}

function text(value) {
  return value === null || value === undefined ? "" : String(value)
}

function findBy(rows, field, id) {
  var key = text(id)
  if (key === "") return null
  var all = list(rows)
  for (var i = 0; i < all.length; i++)
    if (all[i] && text(all[i][field]) === key) return all[i]
  return null
}

function number(value) {
  return typeof value === "number" && isFinite(value) ? value : null
}

function trimmed(value, digits) {
  var scale = Math.pow(10, digits)
  return String(Math.round(value * scale) / scale)
}

// A server-provided `scope_label` wins; otherwise the loaded lists name the
// scope, and an unresolved one (e.g. a task not on today's plan) falls back to
// a generic "Task"/"Epic"/"Project" label so no chip is blank.
function scopeLabel(goal, ctx) {
  var strings = ctx.strings || {}
  var hit = null
  if (goal.scope_label) return text(goal.scope_label)
  if (goal.item_id) {
    hit = findBy(ctx.items, "id", goal.item_id)
    return hit ? text(hit.title) : text(strings.task)
  }
  if (goal.epic_id) {
    hit = findBy(ctx.epics, "id", goal.epic_id)
    return hit ? text(hit.name) : text(strings.epic)
  }
  if (goal.project_id) {
    hit = findBy(ctx.projects, "id", goal.project_id)
    return hit ? text(hit.name) : text(strings.project)
  }
  return text(ctx.strings && ctx.strings.allWork)
}

function metricInfo(goal, ctx) {
  var id = text(goal.metric_id)
  if (id === "") return null
  // metrics.list rows carry `id`; project asks carry `metric_id`.
  var hit = findBy(ctx.metrics, "id", id) || findBy(ctx.metrics, "metric_id", id)
  return { name: hit && hit.name ? text(hit.name) : id, unit: hit ? text(hit.unit) : "" }
}

function chipFor(goal, ctx) {
  var strings = ctx.strings || {}
  var progress = goal.progress || {}
  var metric = metricInfo(goal, ctx)
  var target = number(goal.target)
  var value = number(progress.value)
  var atMost = goal.direction === "at_most"
  var fraction = 0
  if (value !== null && target !== null && target > 0) fraction = Math.min(1, Math.max(0, value / target))

  var label = scopeLabel(goal, ctx)
  if (metric) label = label === "" ? metric.name : label + " · " + metric.name
  var targetText = target === null ? "" : (metric ? trimmed(target, 1) : String(Math.round(target)))
  var valueText = progress.error === "sum_overflow" ? text(strings.sumOverflow)
    : value === null ? text(strings.none)
    : (metric ? trimmed(value, 1) : String(Math.round(value)))
  var unit = metric ? metric.unit : text(strings.minuteUnit)
  var amount = valueText + " / " + (atMost ? "≤ " : "") + targetText + (unit === "" ? "" : " " + unit)
  return {
    id: text(goal.id),
    period: goal.period === "week" ? "week" : "day",
    label: label,
    amount: amount,
    fraction: fraction,
    met: progress.met === true,
    atMost: atMost
  }
}

// Today's goals first, then this week's, keeping the server position order
// within each group. `ctx`: {projects, epics, items, metrics, strings:{allWork,
// task, epic, project, none, sumOverflow, minuteUnit}}.
// Returns {chips: first MAX_CHIPS, more: count hidden}.
function goalChips(goals, ctx, max) {
  var limit = max === undefined ? MAX_CHIPS : max
  var rows = list(goals).filter(function(goal) {
    return goal && goal.paused !== true && (goal.period === "day" || goal.period === "week")
  })
  var day = rows.filter(function(goal) { return goal.period === "day" })
  var week = rows.filter(function(goal) { return goal.period === "week" })
  var ordered = day.concat(week)
  var chips = ordered.slice(0, limit).map(function(goal) { return chipFor(goal, ctx || {}) })
  return { chips: chips, more: Math.max(0, ordered.length - chips.length) }
}
