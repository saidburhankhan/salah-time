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

test("project tags filter the catalogue and never become item fields", () => {
  const [quran] = fixture("projects_list")
  const projects = [quran, { ...quran, id: "second" }, { id: "untagged", tags: [], tag_ids: [] }]
  same(logic.tagsForProjects(projects), quran.tags)
  same(logic.projectsForTag(projects, quran.tag_ids[0]).map(p => p.id), [quran.id, "second"])
  same(logic.projectsForTag(projects, "").map(p => p.id), [quran.id, "second", "untagged"])
})

test("the sheet sends the parsed title, recurrence and selected schedule", () => {
  const weekly = { ...daily, repeat: "weekly", weekdays: ["mon", "fri"] }
  same(logic.addParams(weekly, { minutes: 30, block: "asr" }, "project", false, "2031-05-15"),
    { title: daily.title, planned_minutes: 30, project_id: "project", date: "2031-05-15",
      block: "asr", repeat: "weekly", weekdays: ["mon", "fri"] })
  same(logic.addParams(once, { minutes: null, block: "isha" }, "", false, "2031-05-14"),
    { title: once.title, date: "2031-05-14", block: "isha" })
})

test("backlog has no day or block and only accepts a parsed one-time task", () => {
  const fields = { minutes: 20, block: "isha" }
  same(logic.addParams(once, fields, "", true, "2031-05-15"),
    { title: once.title, planned_minutes: 20, backlog: true })
  assert.equal(logic.addParams(daily, fields, "", true, ""), null)
  assert.equal(logic.addParams(null, fields, "", true, ""), null)
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
  assert.equal(logic.addParams(draft, { minutes: 20, block: "isha" }, "", false, schedule.date).date,
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
  const withEpic = logic.addParams(draft, { minutes: 15, block: "fajr" }, "p", false, "2031-05-14", "e")
  assert.equal(withEpic.epic_id, "e")
  assert.equal(logic.addParams(draft, {}, "p", false, "", "").epic_id, undefined)
  assert.equal(logic.addParams(draft, {}, "", false, "", "e").epic_id, undefined)
  assert.equal(logic.addParams(draft, {}, "p", true, "", "e").epic_id, "e")
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
