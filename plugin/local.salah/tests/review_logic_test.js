#!/usr/bin/env node
"use strict"

const assert = require("node:assert/strict")
const fs = require("node:fs")
const path = require("node:path")
const vm = require("node:vm")
const { test } = require("node:test")

const logic = {}
const filename = path.join(__dirname, "..", "ReviewLogic.js")
vm.runInNewContext(fs.readFileSync(filename, "utf8").replace(/^\.pragma library\s*/, ""), logic, { filename })

function status(date, passed, day = date) {
  return { date, day, prayers: [{ id: "fajr", passed }] }
}

test("Monday after Fajr requests the prior calendar Monday", () => {
  assert.equal(logic.previousWeek(status("2031-05-12", true)), "2031-05-05")
  assert.equal(logic.previousWeek(status("2024-01-01", true)), "2023-12-25")
  assert.equal(logic.previousWeek(status("2024-03-04", true)), "2024-02-26")
})

test("Monday before Fajr and other calendar days are ineligible", () => {
  assert.equal(logic.previousWeek(status("2031-05-12", false, "2031-05-11")), "")
  assert.equal(logic.previousWeek(status("2031-05-13", false, "2031-05-12")), "")
  assert.equal(logic.previousWeek(status("2031-05-13", true)), "")
})

test("incomplete or malformed prayer snapshots cannot prompt a review", () => {
  assert.equal(logic.previousWeek(null), "")
  assert.equal(logic.previousWeek({ date: "2031-05-12", prayers: [{ id: "isha", passed: true }] }), "")
  assert.equal(logic.previousWeek(status("2025-02-31", true)), "")
})

test("only a reply for the requested week with an explicit null note can show the link", () => {
  const week = "2031-05-05"
  assert.equal(logic.hasNoNote(week, { week, note: null }), true)
  assert.equal(logic.hasNoNote(week, { week, note: "More breaks next week." }), false)
  assert.equal(logic.hasNoNote(week, { week }), false)
  assert.equal(logic.hasNoNote(week, null), false)
  assert.equal(logic.hasNoNote(week, { week: "2031-04-28", note: null }), false)
  assert.equal(logic.hasNoNote("", { week, note: null }), false)
})
