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
