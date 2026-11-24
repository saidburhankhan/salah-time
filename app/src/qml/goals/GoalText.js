.pragma library

// Text and tints for a GoalView row. `win` is Main.qml (metric lookup,
// duration and number formatting); nothing here talks to the backend.

function metric(win, goal) {
    return goal.metric_id ? win.metricById(goal.metric_id) : null
}

function metricName(win, goal) {
    if (!goal.metric_id)
        return qsTr("Minutes")
    var m = metric(win, goal)
    return m ? String(m.name || goal.metric_id) : String(goal.metric_id)
}

// A value or target in the goal's own unit: time for minutes goals, the
// metric's number and unit otherwise. No value is "—", never zero.
function amount(win, goal, value) {
    if (value === null || value === undefined || isNaN(Number(value)))
        return "—"
    if (!goal.metric_id)
        return win.durationText(value)
    var m = metric(win, goal)
    var unit = m && m.unit ? " " + String(m.unit) : ""
    return win.numberText(value) + unit
}

function progressAmount(win, goal, progress) {
    if (progress.error === "sum_overflow")
        return qsTr("Sum too large")
    return amount(win, goal, progress.value)
}

function periodText(goal, day, week) { return goal.period === "week" ? week : day }

function directionText(goal, atLeast, atMost) { return goal.direction === "at_most" ? atMost : atLeast }

// 0..1 fill of the progress bar; an at-most goal fills towards its limit.
function ratio(goal) {
    var progress = goal.progress || {}
    var value = Number(progress.value)
    var target = Number(goal.target)
    if (progress.value === null || progress.value === undefined || isNaN(value) || !(target > 0))
        return 0
    return Math.max(0, Math.min(1, value / target))
}

// Muted tints for the dark surface: green met, red over an at-most limit,
// amber still short of an at-least target.
function tint(goal) {
    var progress = goal.progress || {}
    if (progress.met)
        return "#6c9a64"
    return goal.direction === "at_most" ? "#b5574a" : "#c49a45"
}
