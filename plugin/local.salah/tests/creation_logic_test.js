#!/usr/bin/env node
"use strict"

const assert = require("node:assert/strict")
const fs = require("node:fs")
const path = require("node:path")
const vm = require("node:vm")
const { test } = require("node:test")

const logic = {}
const filename = path.join(__dirname, "..", "CreationLogic.js")
vm.runInNewContext(fs.readFileSync(filename, "utf8").replace(/^\.pragma library\s*/, ""), logic, { filename })

function fixture(name) {
  const dir = path.join(__dirname, "../../../crates/salah-cli/tests/fixtures/rpc")
  return JSON.parse(fs.readFileSync(path.join(dir, name + ".json"), "utf8")).result
}

function same(actual, expected) {
  assert.deepEqual(JSON.parse(JSON.stringify(actual)), expected)
}

const daily = fixture("item_parse")
const once = { ...daily, block: "isha", repeat: "none" }

test("the parse result prefills Isha and a custom 20-minute chip", () => {
  same(logic.prefill(once, null, "", "", "asr"), { block: "isha", minutes: 20 })
  same(logic.minuteChoices(once), [15, 20, 30, 60, 90])
  same(logic.minuteChoices({ planned_minutes: 15 }), [15, 30, 60, 90])
})

test("explicit choices override prefill and a block-row preset overrides the parsed block", () => {
  same(logic.prefill(daily, 60, "asr", "night", "isha"), { block: "asr", minutes: 60 })
  same(logic.prefill(daily, null, "", "night", "isha"), { block: "night", minutes: 20 })
  same(logic.prefill({ block: null, planned_minutes: null }, null, "", "", "isha"),
    { block: "isha", minutes: null })
})

test("project defaults are selected before the tag catalogue loads", () => {
  const [quran] = fixture("projects_list")
  const projects = [quran, { id: "untagged", tag_ids: [] }]
  const defaults = logic.defaultTagIds(projects, quran.id)
  same(logic.taskTagIds(defaults, null, []), quran.tag_ids)
  same(logic.defaultTagIds(projects, "untagged"), [])
  same(logic.defaultTagIds(projects, ""), [])
  same(logic.defaultTagIds(projects, "archived"), [])
})

test("task tags can clear all defaults or include tags from outside the chosen project", () => {
  const [quran] = fixture("projects_list")
  const catalogue = [...quran.tags, { id: "standalone", name: "Arabic" }]
  const defaults = logic.defaultTagIds([quran], quran.id)
  const empty = logic.toggleTag(defaults, defaults[0])
  const selected = logic.toggleTag(empty, "standalone")

  same(logic.taskTagIds(defaults, empty, catalogue), [])
  same(logic.taskTagIds(defaults, selected, catalogue), ["standalone"])
  same(logic.taskTagIds(defaults, selected, quran.tags), [])
  same(quran.tag_ids, [quran.tags[0].id])
  same(logic.addParams(once, {}, quran.id, false, "", "", empty),
    { title: once.title, project_id: quran.id, tag_ids: [] })
  same(logic.addParams(once, {}, quran.id, false, "", "", selected),
    { title: once.title, project_id: quran.id, tag_ids: ["standalone"] })
  // Untouched chips: no tag_ids, so the daemon applies the project's current defaults.
  same(logic.addParams(once, {}, quran.id, false, "", "", null),
    { title: once.title, project_id: quran.id })
})

test("the sheet sends the parsed title, recurrence and selected schedule", () => {
  const weekly = { ...daily, repeat: "weekly", weekdays: ["mon", "fri"] }
  same(logic.addParams(weekly, { minutes: 30, block: "asr" }, "project", false, "2031-05-15", "", []),
    { title: daily.title, planned_minutes: 30, project_id: "project", date: "2031-05-15",
      block: "asr", repeat: "weekly", weekdays: ["mon", "fri"], tag_ids: [] })
  same(logic.addParams(once, { minutes: null, block: "isha" }, "", false, "2031-05-14", "", []),
    { title: once.title, date: "2031-05-14", block: "isha", tag_ids: [] })
})

test("backlog has no day or block and only accepts a parsed one-time task", () => {
  const fields = { minutes: 20, block: "isha" }
  const [item] = fixture("item_add_tags").changed.items
  same(logic.addParams(once, fields, "", true, "2031-05-15", "", item.tag_ids),
    { title: once.title, planned_minutes: 20, backlog: true, tag_ids: item.tag_ids })
  assert.equal(logic.addParams(daily, fields, "", true, "", "", []), null)
  assert.equal(logic.addParams(null, fields, "", true, "", "", []), null)
})

test("Tomorrow follows the tracking day across month, leap-day and year boundaries", () => {
  assert.equal(logic.tomorrow("2028-02-28"), "2028-02-29")
  assert.equal(logic.tomorrow("2028-02-29"), "2028-03-01")
  assert.equal(logic.tomorrow("2027-12-31"), "2028-01-01")
  assert.equal(logic.tomorrow(""), "")
})

test("a parsed day prefills the When chip and the date submitted by Add", () => {
  const today = "2028-02-29"
  const draft = { ...once, start_day: "2028-03-01", day: today }
  const schedule = logic.schedule(draft, "", today)
  same(schedule, { when: "tomorrow", date: "2028-03-01" })
  same(logic.schedule({ day: "2028-03-01" }, "", today), { when: "tomorrow", date: "2028-03-01" })
  same(logic.schedule({ day: today }, "", today), { when: "today", date: today })
  same(logic.schedule(once, "", today), { when: "today", date: today })
  assert.equal(logic.addParams(draft, { minutes: 20, block: "isha" }, "", false, schedule.date, "", []).date,
    draft.start_day)
})

test("explicit When choices override the parsed day", () => {
  const today = "2028-02-29"
  same(logic.schedule({ day: "2028-03-01" }, "today", today), { when: "today", date: today })
  same(logic.schedule({ day: today }, "tomorrow", today), { when: "tomorrow", date: "2028-03-01" })
  same(logic.schedule({ day: "2028-03-01" }, "backlog", today), { when: "backlog", date: "" })
})

test("only the project's active epics are offered, in position order", () => {
  const listed = fixture("epics_list")
  const projectId = listed[0].project_id
  const epics = [
    { id: "b", project_id: projectId, name: "Second", status: "active", position: 1 },
    { ...listed[0], status: "active", position: 0 },
    { id: "f", project_id: projectId, name: "Done", status: "finished", position: 2 },
    { id: "o", project_id: "other", name: "Other", status: "active", position: 0 }
  ]
  same(logic.epicsForProject(epics, projectId).map(e => e.name), ["Read", "Second"])
  same(logic.epicsForProject(epics, ""), [])
  same(logic.epicsForProject(null, projectId), [])
})

test("a stale or foreign epic choice falls back to No epic", () => {
  const epics = [
    { id: "a", project_id: "p", name: "A", status: "active", position: 0 },
    { id: "f", project_id: "p", name: "F", status: "finished", position: 1 }
  ]
  assert.equal(logic.validEpicId(epics, "p", "a"), "a")
  assert.equal(logic.validEpicId(epics, "p", "f"), "")
  assert.equal(logic.validEpicId(epics, "q", "a"), "")
  assert.equal(logic.validEpicId(epics, "p", ""), "")
})

test("epic_id is sent with item.add only alongside its project", () => {
  const draft = { title: "Read", repeat: "none" }
  const withEpic = logic.addParams(draft, { minutes: 15, block: "fajr" }, "p", false, "2031-05-14", "e", [])
  assert.equal(withEpic.epic_id, "e")
  assert.equal(logic.addParams(draft, {}, "p", false, "", "", []).epic_id, undefined)
  assert.equal(logic.addParams(draft, {}, "", false, "", "e", []).epic_id, undefined)
  assert.equal(logic.addParams(draft, {}, "p", true, "", "e", []).epic_id, "e")
})

test("day rows name their epic, finished ones included", () => {
  const epics = [{ id: "a", name: "Spring", status: "finished" }]
  assert.equal(logic.epicName(epics, "a"), "Spring")
  assert.equal(logic.epicName(epics, "zz"), "")
  assert.equal(logic.epicName(epics, null), "")
})

test("addedItem reads the socket envelope and the CLI fallback raw text", () => {
  const item = { id: "i1", next_occurrence: "2026-11-24" }
  same(logic.addedItem({ changed: { items: [item] } }), item)
  same(logic.addedItem({ fallback: true, raw: JSON.stringify({ changed: { items: [item] } }) + "\n" }), item)
  assert.equal(logic.addedItem({ fallback: true, raw: "oops" }), null)
  assert.equal(logic.addedItem({ changed: { items: [] } }), null)
  assert.equal(logic.addedItem(null), null)
})

test("clearing a title stops parsing but keeps an open draft until dismissal", () => {
  same(logic.titleEdit("", false), { expanded: false, needsParse: false })
  const initial = logic.titleEdit("Read", false)
  const empty = logic.titleEdit("  ", initial.expanded)
  same(initial, { expanded: true, needsParse: true })
  same(empty, { expanded: true, needsParse: false })
  same(logic.titleEdit("Read another chapter", empty.expanded), { expanded: true, needsParse: true })
  same(logic.titleEdit("", false), { expanded: false, needsParse: false })
})

test("an ambiguous task failure replays its first payload after the planner crosses Sleep", () => {
  let nextId = 0
  let built = 0
  let today = "2031-05-14"
  const tags = ["faith"]
  const newOpId = () => "op-" + ++nextId
  const buildParams = () => {
    built++
    const schedule = logic.schedule(once, "today", today)
    return logic.addParams(once, { block: "isha", minutes: 20 }, "project", false, schedule.date, "", tags)
  }
  const first = logic.itemAttempt(null, buildParams, newOpId)
  assert.equal(first.id, "op-1")

  today = "2031-05-15"
  tags.push("new-tag")
  const uncertain = logic.itemAttemptAfterFailure(first, {})
  const retry = logic.itemAttempt(uncertain, buildParams, newOpId)
  assert.equal(retry, first)
  assert.equal(built, 1)
  assert.equal(retry.params.date, "2031-05-14")
  same(retry.params.tag_ids, ["faith"])

  const item = logic.addedItem({ changed: { items: [{ id: "original-item" }] } })
  const batch = logic.resourceBatch(item.id, [{ kind: "note", body: "Read again" }], newOpId)
  same(batch.pending[0], { id: "op-2", params: { item_id: "original-item", kind: "note", body: "Read again" } })
})

test("a definite task rejection allows a corrected submission with a new operation ID", () => {
  let nextId = 0
  const newOpId = () => "op-" + ++nextId
  const first = logic.itemAttempt(null, () => ({ title: "Read", date: "2031-05-14" }), newOpId)
  assert.equal(logic.itemAttemptAfterFailure(first, { kind: "storage" }), first)
  assert.equal(logic.itemAttemptAfterFailure(first, { kind: "busy" }), first)
  const rejected = logic.itemAttemptAfterFailure(first, { kind: "validation", field: "date" })
  assert.equal(rejected, null)
  const corrected = logic.itemAttempt(rejected, () => ({ title: "Read", date: "2031-05-15" }), newOpId)
  same(corrected, { id: "op-2", params: { title: "Read", date: "2031-05-15" } })
})

test("resource writes use the confirmed item ID and only failed writes are retried with their original IDs", () => {
  let nextId = 0
  const newOpId = () => "op-" + ++nextId
  const drafts = [
    { kind: "note", body: "Notes" },
    { kind: "link", url: "https://example.org" },
    { kind: "reminder", remind_at: "2031-05-15T07:00:00Z" }
  ]
  const started = logic.resourceBatch("new-item", drafts, newOpId)
  same(started.pending.map(write => write.params), drafts.map(draft => ({ item_id: "new-item", ...draft })))
  assert.equal(drafts[0].item_id, undefined)

  const noteSaved = logic.advanceResourceBatch(started, null)
  const linkFailed = logic.advanceResourceBatch(noteSaved, "disconnected")
  same(linkFailed.pending.map(write => write.id), ["op-3"])
  const reminderSaved = logic.advanceResourceBatch(linkFailed, null)
  const retry = logic.retryResourceBatch(reminderSaved)
  same(retry.pending, [{ ...started.pending[1], params: { ...started.pending[1].params }, error: "disconnected" }])
  same(logic.advanceResourceBatch(retry, null), { pending: [], failed: [] })
  assert.equal(nextId, 3)
})
