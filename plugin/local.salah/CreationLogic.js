.pragma library

function prefill(parsed, minutesChoice, blockChoice, presetBlock, defaultBlock) {
  return {
    minutes: minutesChoice !== null ? minutesChoice : (parsed ? parsed.planned_minutes : null),
    block: blockChoice || presetBlock || (parsed && parsed.block) || defaultBlock || ""
  }
}

function minuteChoices(parsed) {
  var choices = [15, 30, 60, 90]
  var minutes = parsed && parsed.planned_minutes
  if (minutes != null && choices.indexOf(minutes) === -1) {
    choices.push(minutes)
    choices.sort(function(a, b) { return a - b })
  }
  return choices
}

function tagsForProjects(projects) {
  var tags = []
  var seen = new Set()
  projects.forEach(function(project) {
    var projectTags = project.tags || []
    projectTags.forEach(function(tag) {
      if (seen.has(tag.id)) return
      seen.add(tag.id)
      tags.push(tag)
    })
  })
  return tags
}

function projectsForTag(projects, tagId) {
  return projects.filter(function(project) {
    return !tagId || (project.tag_ids || []).indexOf(tagId) >= 0
  })
}

// Shift the planner's calendar date even when the machine is in another zone.
function tomorrow(day) {
  if (!day) return ""
  var date = new Date(day + "T00:00:00Z")
  if (!isFinite(date.getTime())) return ""
  date.setUTCDate(date.getUTCDate() + 1)
  return date.toISOString().slice(0, 10)
}

function addParams(parsed, fields, projectId, backlog, date) {
  if (!parsed || !parsed.title || (backlog && parsed.repeat !== "none")) return null
  var params = { title: parsed.title }
  if (fields.minutes != null) params.planned_minutes = fields.minutes
  if (projectId) params.project_id = projectId
  if (backlog) {
    params.backlog = true
  } else {
    if (date) params.date = date
    if (fields.block) params.block = fields.block
    if (parsed.repeat !== "none") params.repeat = parsed.repeat
    if (parsed.weekdays) params.weekdays = parsed.weekdays
  }
  return params
}
