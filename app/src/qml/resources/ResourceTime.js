.pragma library

// Reminder times are RFC 3339 UTC instants on the wire and local date and
// time text in the form.
function two(n) { return (n < 10 ? "0" : "") + n }

function dateText(d) { return d.getFullYear() + "-" + two(d.getMonth() + 1) + "-" + two(d.getDate()) }
function timeText(d) { return two(d.getHours()) + ":" + two(d.getMinutes()) }

// Local "YYYY-MM-DD" and "HH:MM" to a UTC instant, or "" when not a real time.
function toInstant(date, time) {
    var d = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(date).trim())
    var t = /^(\d{2}):(\d{2})$/.exec(String(time).trim())
    if (!d || !t) return ""
    var value = new Date(+d[1], +d[2] - 1, +d[3], +t[1], +t[2])
    if (isNaN(value.getTime()) || dateText(value) !== d[0] || timeText(value) !== t[0]) return ""
    return value.toISOString().replace(/\.\d{3}Z$/, "Z")
}

// Today or tomorrow at a local hour, as an instant.
function daysFromNow(days, hour) {
    var d = new Date()
    d.setDate(d.getDate() + days)
    d.setHours(hour, 0, 0, 0)
    return d
}

function isOverdue(instant, done, now) {
    if (done || !instant) return false
    var d = new Date(instant)
    return !isNaN(d.getTime()) && d.getTime() < (now || new Date()).getTime()
}

function display(instant) {
    var d = new Date(instant)
    if (isNaN(d.getTime())) return ""
    return dateText(d) + "  " + timeText(d)
}

// A link needs an absolute http or https URL.
function validUrl(text) { return /^https?:\/\/[^\s\/?#]+[^\s]*$/i.test(String(text).trim()) }
