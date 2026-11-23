.pragma library

// Status decides which reminders are due. Sort a copy so rendering never
// modifies the status snapshot. Keep the server order for equal instants.
function orderedDueReminders(rows) {
  if (!Array.isArray(rows)) return []
  return rows.slice().sort(function(a, b) {
    return Date.parse(a.remind_at) - Date.parse(b.remind_at)
  })
}
