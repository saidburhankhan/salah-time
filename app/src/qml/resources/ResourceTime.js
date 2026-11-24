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

function webUrl(value) {
  if (typeof value !== "string") return ""
  var url = value.trim()
  if (!/^https?:\/\/[^/?#]+/i.test(url) || /[\s\\\x00-\x1f\x7f-\x9f]/.test(url)) return ""
  try {
    var parsed = new URL(url)
    return (parsed.protocol === "http:" || parsed.protocol === "https:") && parsed.hostname ? url : ""
  } catch (error) {
    return ""
  }
}

function normalizedWebUrl(value) {
  if (typeof value !== "string") return ""
  var input = value.trim()
  if (/^https?:\/\//i.test(input)) return webUrl(input)
  // An omitted scheme is only unambiguous for a domain (or IPv4 address).
  // Keep relative paths, credentials and other schemes out of this path.
  var authority = /^([^:/?#@]+)(?::[0-9]+)?(?:[/?#]|$)/.exec(input)
  if (!authority) return ""
  var labels = authority[1].replace(/\.$/, "").split(".")
  if (labels.length < 2 || !labels.every(function(label) {
    return /^[a-z0-9\u00a1-\uffff](?:[a-z0-9\u00a1-\uffff-]*[a-z0-9\u00a1-\uffff])?$/i.test(label)
  })) return ""
  return webUrl("https://" + input)
}

function validUrl(text) { return normalizedWebUrl(text) !== "" }
