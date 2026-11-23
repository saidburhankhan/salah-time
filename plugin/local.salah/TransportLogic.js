.pragma library

function followUpStatusRevision(completed, applied, pending, clockPending) {
  var latest = Math.max(applied, pending)
  return latest > completed || clockPending ? Math.max(completed, latest) : null
}
