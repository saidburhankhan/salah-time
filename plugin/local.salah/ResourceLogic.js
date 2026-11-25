.pragma library

// Status decides which reminders are due. Sort a copy so rendering never
// modifies the status snapshot. Keep the server order for equal instants.
function orderedDueReminders(rows) {
  if (!rows || typeof rows.length !== "number") return []
  return Array.prototype.slice.call(rows).sort(function(a, b) {
    return Date.parse(a.remind_at) - Date.parse(b.remind_at)
  })
}

function resourceCount(counts) {
  if (!counts) return 0
  return (counts.notes || 0) + (counts.links || 0) + (counts.reminders || 0)
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

function reminderInstant(date, time) {
  var day = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date.trim())
  var clock = /^(\d{2}):(\d{2})$/.exec(time.trim())
  if (!day || !clock) return ""
  var instant = new Date(day[0] + "T" + clock[0] + ":00")
  // Reject rollover dates and times skipped by a daylight-saving change.
  if (instant.getFullYear() !== Number(day[1]) || instant.getMonth() + 1 !== Number(day[2])
      || instant.getDate() !== Number(day[3]) || instant.getHours() !== Number(clock[1])
      || instant.getMinutes() !== Number(clock[2])) return ""
  return instant.toISOString()
}

function resourceDraft(kind, value, date, time) {
  var text = value.trim()
  if (kind === "note" && text && Array.from(text).length <= 5000) return { kind: kind, body: text }
  if (kind === "link") {
    var url = normalizedWebUrl(value)
    if (url && Array.from(url).length <= 2000) return { kind: kind, url: url }
  }
  if (kind === "reminder" && typeof date === "string" && typeof time === "string") {
    var when = reminderInstant(date, time)
    if (when) {
      var params = { kind: kind, remind_at: when }
      if (text) params.title = text
      return params
    }
  }
  return null
}

function quickResourceParams(itemId, kind, value) {
  if (!itemId) return null
  var draft = resourceDraft(kind, value)
  return draft ? Object.assign({ item_id: itemId }, draft) : null
}
