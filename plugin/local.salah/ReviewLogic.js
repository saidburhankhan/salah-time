.pragma library

function previousWeek(status) {
  if (!status || typeof status.date !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(status.date))
    return ""
  // The status supplies the calendar date and Fajr state for the configured
  // location. UTC here only keeps calendar arithmetic independent of the host.
  var monday = new Date(status.date + "T00:00:00Z")
  if (monday.getUTCDay() !== 1 || monday.toISOString().slice(0, 10) !== status.date)
    return ""
  if (!(status.prayers || []).some(function(prayer) {
    return prayer && prayer.id === "fajr" && prayer.passed === true
  })) return ""
  monday.setUTCDate(monday.getUTCDate() - 7)
  return monday.getUTCFullYear() >= 0 ? monday.toISOString().slice(0, 10) : ""
}

function hasNoNote(week, review) {
  return !!week && !!review && review.week === week && review.note === null
}
