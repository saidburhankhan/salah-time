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

function quickResourceParams(itemId, kind, value) {
  if (!itemId) return null
  if (kind === "note") {
    var body = value.trim()
    if (body) return { item_id: itemId, kind: kind, body: body }
  } else if (kind === "link") {
    var url = webUrl(value)
    if (url) return { item_id: itemId, kind: kind, url: url }
  }
  return null
}
