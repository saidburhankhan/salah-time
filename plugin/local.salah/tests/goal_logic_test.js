#!/usr/bin/env node
"use strict"

const assert = require("node:assert/strict")
const fs = require("node:fs")
const path = require("node:path")
const vm = require("node:vm")
const { test } = require("node:test")

const logic = {}
const filename = path.join(__dirname, "..", "GoalLogic.js")
vm.runInNewContext(fs.readFileSync(filename, "utf8").replace(/^\.pragma library\s*/, ""), logic, { filename })

const strings = { allWork: "All work", task: "Task", epic: "Epic", project: "Project",
  none: "—", sumOverflow: "Total too large", minuteUnit: "min" }
const status = JSON.parse(fs.readFileSync(path.join(__dirname,
  "../../../crates/salah-cli/tests/fixtures/rpc/status_goals.json"), "utf8")).result
const ctx = {
  projects: [{ id: "<id:3>", name: "Study" }],
  epics: [{ id: "<id:4>", name: "Thesis" }],
  items: status.planner.items,
  metrics: [{ id: "focus", name: "Focus", unit: "" }, { id: "distractions", name: "Distractions", unit: "times" }, { id: "pages", name: "Pages", unit: "pages" }],
  strings
}

function goal(extra) {
  return Object.assign({ id: "g", period: "day", direction: "at_least", target: 60, metric_id: null,
    project_id: null, epic_id: null, item_id: null, progress: { value: 30, met: false } }, extra)
}

test("fixture: today's goals first, then week, at most 3 with +N", () => {
  const out = logic.goalChips(status.goals, ctx)
  assert.equal(out.chips.length, 3)
  assert.equal(out.more, 3)
  assert.equal(out.chips.map(c => c.period).join(),"day,day,day")
  assert.equal(out.chips[0].label, "All work")
  assert.equal(out.chips[0].amount, "62 / 60 min")
  assert.equal(out.chips[1].label, "Study")
  assert.equal(out.chips[2].label, "Read")
  assert.ok(out.chips.every(c => c.met && c.fraction === 1))
})

test("fixture: week goals, metric names, at_most unmet", () => {
  const out = logic.goalChips(status.goals, ctx, 10)
  assert.equal(out.more, 0)
  const byId = Object.fromEntries(out.chips.map(c => [c.id, c]))
  assert.equal(byId["<id:5>"].label, "Book")
  assert.equal(byId["<id:8>"].label, "All work · Focus")
  assert.equal(byId["<id:8>"].amount, "4.4 / 4")
  assert.equal(byId["<id:8>"].fraction, 1)
  const most = byId["<id:9>"]
  assert.equal(most.label, "Study · Distractions")
  assert.equal(most.amount, "4 / ≤ 3 times")
  assert.equal(most.met, false)
  assert.equal(most.atMost, true)
  assert.equal(most.fraction, 1)
})

test("empty and missing goals give no chips", () => {
  for (const input of [[], null, undefined, "x"]) { const o = logic.goalChips(input, ctx); assert.equal(o.chips.length, 0); assert.equal(o.more, 0) }
})

test("null metric value shows the placeholder and an empty bar", () => {
  const out = logic.goalChips([goal({ metric_id: "focus", target: 4, progress: { value: null, met: false } })], ctx)
  assert.equal(out.chips[0].amount, "— / 4")
  assert.equal(out.chips[0].fraction, 0)
  assert.equal(out.chips[0].met, false)
})

test("a sum overflow gives a distinct message and keeps the target visible", () => {
  const limit = { metric_id: "distractions", direction: "at_most", target: 3 }
  const empty = { value: null, met: false }
  const out = logic.goalChips([
    goal({ ...limit, progress: { ...empty, error: "sum_overflow" } }),
    goal({ ...limit, progress: empty })
  ], ctx)
  assert.equal(out.chips[0].amount, "Total too large / ≤ 3 times")
  assert.equal(out.chips[0].fraction, 0)
  assert.equal(out.chips[0].met, false)
  assert.equal(out.chips[1].amount, "— / ≤ 3 times")
})

test("unmet progress fraction and unknown scope", () => {
  const out = logic.goalChips([goal({ project_id: "gone" })], ctx)
  assert.equal(out.chips[0].fraction, 0.5)
  assert.equal(out.chips[0].label, "Project")
})

test("array-like input from QML is accepted", () => {
  const out = logic.goalChips({ length: 1, 0: goal({}) }, ctx)
  assert.equal(out.chips.length, 1)
})

test("unknown periods are ignored", () => {
  assert.equal(logic.goalChips([goal({ period: "month" })], ctx).chips.length, 0)
})

test("an unresolved scope falls back to a generic label", () => {
  const empty = Object.assign({}, ctx, { items: [], epics: [], projects: [] })
  const labels = logic.goalChips([goal({ item_id: "x" }), goal({ epic_id: "y" }), goal({ project_id: "z" })], empty)
    .chips.map(chip => String(chip.label))
  assert.deepEqual([...labels], ["Task", "Epic", "Project"])
})

test("fixture scope labels win over stale project, epic and task names", () => {
  const stale = {
    ...ctx,
    projects: [{ id: "<id:3>", name: "Old project" }],
    epics: [{ id: "<id:4>", name: "Old epic" }],
    items: [{ id: "<id:7>", title: "Old task" }]
  }
  const { chips } = logic.goalChips(status.goals, stale, 10)
  assert.deepEqual(Array.from(chips, chip => chip.label),
    ["All work", "Study", "Read", "Book", "All work · Focus", "Study · Distractions"])
})

test("metric goals find the catalogue row by metrics.list id (custom metric name and unit)", () => {
  const chip = logic.goalChips([goal({ metric_id: "pages", target: 20, progress: { value: 12, met: false } })], ctx).chips[0]
  assert.equal(chip.label, "All work · Pages")
  assert.equal(chip.amount, "12 / 20 pages")
})
