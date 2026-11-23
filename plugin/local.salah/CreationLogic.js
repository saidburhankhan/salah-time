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

function defaultTagIds(projects, projectId) {
  var project = projects.find(function(row) { return row.id === projectId })
  return project ? (project.tag_ids || []) : []
}

function taskTagIds(defaults, choice, tags) {
  if (choice === null) return defaults
  return choice.filter(function(id) {
    return tags.some(function(tag) { return tag.id === id })
  })
}

function toggleTag(tagIds, id) {
  return tagIds.includes(id) ? tagIds.filter(function(tagId) { return tagId !== id })
    : tagIds.concat(id)
}

// Shift the planner's calendar date even when the machine is in another zone.
function tomorrow(day) {
  if (!day) return ""
  var date = new Date(day + "T00:00:00Z")
  if (!isFinite(date.getTime())) return ""
  date.setUTCDate(date.getUTCDate() + 1)
  return date.toISOString().slice(0, 10)
}

function schedule(parsed, whenChoice, today) {
  var parsedDay = (parsed && (parsed.start_day || parsed.day)) || ""
  var when = whenChoice || (parsedDay && today && parsedDay !== today ? "tomorrow" : "today")
  if (when === "backlog") return { when: when, date: "" }
  var date = (!whenChoice && parsedDay) || (when === "tomorrow" ? tomorrow(today) : today)
  return { when: when, date: date }
}

// A project's active epics in the core's order. Finished epics are not offered.
function epicsForProject(epics, projectId) {
  if (!projectId) return []
  return (epics || []).filter(function(epic) {
    return epic && epic.project_id === projectId && (epic.status || "active") === "active"
  }).sort(function(a, b) { return (Number(a.position) || 0) - (Number(b.position) || 0) })
}

// The chosen epic if it still belongs to the project and is active, else "".
function validEpicId(epics, projectId, epicId) {
  if (!epicId) return ""
  return epicsForProject(epics, projectId).some(function(epic) { return epic.id === epicId }) ? epicId : ""
}

// Name for a day row; finished epics keep their name, an unknown id shows nothing.
function epicName(epics, epicId) {
  if (!epicId) return ""
  var list = epics || []
  for (var i = 0; i < list.length; i++)
    if (list[i] && list[i].id === epicId) return String(list[i].name || "")
  return ""
}

function addParams(parsed, fields, projectId, backlog, date, epicId, tagIds) {
  if (!parsed || !parsed.title || (backlog && parsed.repeat !== "none")) return null
  var params = { title: parsed.title, tag_ids: tagIds }
  if (fields.minutes != null) params.planned_minutes = fields.minutes
  if (projectId) {
    params.project_id = projectId
    if (epicId) params.epic_id = epicId
  }
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

function addedItem(result) {
  if (!result) return null
  if (result.fallback) {
    try {
      result = JSON.parse(result.raw)
    } catch (error) {
      return null
    }
  }
  var items = result && result.changed && result.changed.items
  return items && items.length ? items[0] : null
}
