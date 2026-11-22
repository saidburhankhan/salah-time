#!/usr/bin/env node
"use strict"

// Plain node test for the pure drag rules. The QML helper is a .pragma library
// file, so strip that line and evaluate it in a VM global context.

const fs = require("fs")
const path = require("path")
const vm = require("vm")

const helperPath = path.join(__dirname, "..", "DragLogic.js")
let source = fs.readFileSync(helperPath, "utf8")
source = source.replace(/^\.pragma library\s*/, "")
const context = {}
vm.createContext(context)
vm.runInContext(source, context, { filename: helperPath })

const targetIndex = context.targetIndex
const destination = context.destination

let checks = 0
function assert(condition, message) {
  checks++
  if (!condition) throw new Error("FAIL: " + message)
  console.log("ok " + checks + " - " + message)
}

// ListModel.move(from, to, 1) semantics: remove at `from`, insert at final
// index `to`. This mirrors what BarWidget.qml does.
function moveRow(rows, from, to) {
  const copy = rows.slice()
  const row = copy.splice(from, 1)[0]
  copy.splice(to, 0, row)
  return copy
}

function rows() {
  return [
    { rowType: "header", rowKey: "header:fajr", block: "fajr" },
    { rowType: "item", rowKey: "item:a", block: "fajr", itemId: "a" },
    { rowType: "item", rowKey: "item:b", block: "fajr", itemId: "b" },
    { rowType: "add", rowKey: "add:fajr", block: "fajr" },
    { rowType: "marker", rowKey: "marker:sunrise", block: "fajr" },
    { rowType: "header", rowKey: "header:dhuhr", block: "dhuhr" },
    { rowType: "add", rowKey: "add:dhuhr", block: "dhuhr" },
    { rowType: "header", rowKey: "header:isha", block: "isha" },
    { rowType: "item", rowKey: "item:c", block: "isha", itemId: "c" },
    { rowType: "add", rowKey: "add:isha", block: "isha" },
    { rowType: "marker", rowKey: "marker:sleep", block: "isha" }
  ]
}

function keys(list) {
  return list.map(row => row.rowKey)
}

// Dragging a below Sleep clamps to the end of the last block (just above
// add:isha), never after the Sleep marker.
{
  const list = rows()
  const target = targetIndex(list, "item:a", -1, false, "below")
  assert(target === 8, "below the last marker/below list targets the last block end")
  const moved = moveRow(list, 1, target)
  assert(keys(moved).indexOf("item:a") === 8, "row lands before the last block's add row")
  assert(keys(moved)[9] === "add:isha" && keys(moved)[10] === "marker:sleep",
    "Sleep stays after the last block's add row")
  const dest = destination(moved, "item:a")
  assert(dest.valid === true && dest.block === "isha" && dest.index === 1,
    "drop below Sleep resolves to last block / index 1")
  assert(dest.after_id === "c" && dest.before_id === "",
    "drop below Sleep reports after_id c and no before_id")
}

// Markers never take the gap; the row keeps its current valid position.
{
  const list = rows()
  const overSleep = targetIndex(list, "item:a", 10, false, "")
  assert(overSleep === 1, "hovering the Sleep marker does not move the row")
  const overSunrise = targetIndex(list, "item:c", 4, false, "")
  assert(overSunrise === 8, "hovering the Sunrise marker does not move the row")
  const dest = destination(list, "item:a")
  assert(dest.valid === true && dest.block === "fajr" && dest.index === 0,
    "marker hover keeps a valid destination")
}

// Above the first header clamps into the first block, just below its header.
{
  const list = rows()
  const target = targetIndex(list, "item:c", -1, false, "above")
  assert(target === 1, "above the first header targets just below it")
  const moved = moveRow(list, 8, target)
  assert(keys(moved)[0] === "header:fajr" && keys(moved)[1] === "item:c",
    "row clamps into the first block")
  const dest = destination(moved, "item:c")
  assert(dest.valid === true && dest.block === "fajr" && dest.index === 0,
    "clamped drop resolves into the first block")
  assert(dest.after_id === "" && dest.before_id === "a",
    "clamped drop reports before_id a")
}

// Hovering an empty block's header puts the row into that block.
{
  const list = rows()
  const target = targetIndex(list, "item:a", 5, false, "")
  assert(target === 5, "hovering the dhuhr header targets just below it")
  const moved = moveRow(list, 1, target)
  assert(keys(moved)[4] === "header:dhuhr" && keys(moved)[5] === "item:a"
    && keys(moved)[6] === "add:dhuhr", "row lands inside the empty block")
  const dest = destination(moved, "item:a")
  assert(dest.valid === true && dest.block === "dhuhr" && dest.index === 0,
    "empty block drop resolves to block dhuhr / index 0")
  assert(dest.after_id === "" && dest.before_id === "",
    "empty block has no neighbouring item ids")
}

// Within a block, hovering below an item inserts after it.
{
  const list = rows()
  const target = targetIndex(list, "item:a", 2, true, "")
  assert(target === 2, "hovering below item b targets after b")
  const moved = moveRow(list, 1, target)
  assert(keys(moved)[1] === "item:b" && keys(moved)[2] === "item:a",
    "row reorders within its block")
  const dest = destination(moved, "item:a")
  assert(dest.valid === true && dest.block === "fajr" && dest.index === 1,
    "within-block drop resolves to index 1")
  assert(dest.after_id === "b" && dest.before_id === "",
    "within-block drop reports after_id b")
}

// Hovering above an item inserts before it.
{
  const list = rows()
  const target = targetIndex(list, "item:b", 1, false, "")
  assert(target === 1, "hovering above item a targets before a")
  const moved = moveRow(list, 2, target)
  assert(keys(moved)[1] === "item:b" && keys(moved)[2] === "item:a",
    "row can move above a sibling item")
  const dest = destination(moved, "item:b")
  assert(dest.valid === true && dest.block === "fajr" && dest.index === 0,
    "drop above sibling resolves to index 0")
  assert(dest.after_id === "" && dest.before_id === "a",
    "drop above sibling reports before_id a")
}

// Hovering an add row inserts just above it.
{
  const list = rows()
  const target = targetIndex(list, "item:a", 3, false, "")
  assert(target === 2, "hovering the add row targets just above it")
  const moved = moveRow(list, 1, target)
  assert(keys(moved)[2] === "item:a" && keys(moved)[3] === "add:fajr",
    "row sits immediately above the add row")
}

// Any position after the last block's add row is invalid and must cancel.
{
  const invalid = moveRow(rows(), 1, 9)
  const dest = destination(invalid, "item:a")
  assert(dest.valid === false, "position after the last add row is invalid")
  const missing = destination(rows(), "item:missing")
  assert(missing.valid === false, "missing dragged row is invalid")
  const markerDrop = destination(rows().concat([
    { rowType: "item", rowKey: "item:rogue", block: "isha", itemId: "rogue" }
  ]), "item:rogue")
  assert(markerDrop.valid === false, "row below the Sleep marker is invalid")
}

console.log("\nAll " + checks + " drag logic checks passed")
