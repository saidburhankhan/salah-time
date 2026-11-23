#!/usr/bin/env node
"use strict"

const assert = require("node:assert/strict")
const fs = require("node:fs")
const path = require("node:path")
const vm = require("node:vm")
const { test } = require("node:test")

const logic = {}
const filename = path.join(__dirname, "..", "ResourceLogic.js")
vm.runInNewContext(fs.readFileSync(filename, "utf8").replace(/^\.pragma library\s*/, ""), logic, { filename })

function fixture(name) {
  const dir = path.join(__dirname, "../../../crates/salah-cli/tests/fixtures/rpc")
  return JSON.parse(fs.readFileSync(path.join(dir, name + ".json"), "utf8")).result
}

test("due reminders display oldest first for both item and epic parents", () => {
  const snapshot = fixture("status_reminders").reminders
  const reversed = snapshot.slice().reverse()
  const ordered = logic.orderedDueReminders(reversed)
  assert.deepEqual(Array.from(ordered), snapshot)
  assert.equal(reversed[0], snapshot[1])
})

test("equivalent instants keep the server order within a timestamp", () => {
  const a = { id: "a", remind_at: "2031-05-14T04:25:00Z" }
  const b = { id: "b", remind_at: "2031-05-14T09:25:00+05:00" }
  const later = { id: "later", remind_at: "2031-05-14T04:30:00Z" }
  assert.deepEqual(Array.from(logic.orderedDueReminders([later, a, b])), [a, b, later])
})

test("a status snapshot without reminders has no due rows", () => {
  assert.equal(logic.orderedDueReminders(undefined).length, 0)
  assert.equal(logic.orderedDueReminders(null).length, 0)
})
