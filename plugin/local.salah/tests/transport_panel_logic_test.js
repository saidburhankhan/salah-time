#!/usr/bin/env node
"use strict"

const assert = require("node:assert/strict")
const fs = require("node:fs")
const path = require("node:path")
const vm = require("node:vm")
const { test } = require("node:test")

function load(name) {
  const ctx = {}
  const filename = path.join(__dirname, "..", name)
  vm.runInNewContext(fs.readFileSync(filename, "utf8").replace(/^\.pragma library\s*/, ""), ctx, { filename })
  return ctx
}
const transport = load("TransportLogic.js")
const panel = load("PanelLogic.js")

test("a clock boundary during an in-flight read forces a fresh status.get", () => {
  assert.equal(transport.followUpStatusRevision(5, 5, 0, true), 5)
  assert.equal(transport.followUpStatusRevision(5, 5, 0, false), null)
  assert.equal(transport.followUpStatusRevision(5, 7, 0, false), 7)
  assert.equal(transport.followUpStatusRevision(5, 5, 9, false), 9)
})

test("barStrip covers the bar edge for each position", () => {
  assert.deepEqual({ ...panel.barStrip("top", 100, 50, 10) }, { x: 0, y: 0, width: 100, height: 10 })
  assert.deepEqual({ ...panel.barStrip("bottom", 100, 50, 10) }, { x: 0, y: 40, width: 100, height: 10 })
  assert.deepEqual({ ...panel.barStrip("left", 100, 50, 10) }, { x: 0, y: 0, width: 10, height: 50 })
  assert.deepEqual({ ...panel.barStrip("right", 100, 50, 10) }, { x: 90, y: 0, width: 10, height: 50 })
})
