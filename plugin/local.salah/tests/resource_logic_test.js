#!/usr/bin/env node
"use strict"

const assert = require("node:assert/strict")
const fs = require("node:fs")
const path = require("node:path")
const vm = require("node:vm")
const { test } = require("node:test")

const logic = { URL }
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

test("QML-style array-like reminders are copied and sorted", () => {
  const [earlier, later] = fixture("status_reminders").reminders
  const rows = { 0: later, 1: earlier, length: 2 }
  assert.deepEqual(Array.from(logic.orderedDueReminders(rows)), [earlier, later])
  assert.equal(rows[0], later)
})

test("the paperclip count adds notes, links and open reminders", () => {
  assert.equal(logic.resourceCount({ notes: 2, links: 1, reminders: 4 }), 7)
  assert.equal(logic.resourceCount({ notes: 0, links: 0, reminders: 0 }), 0)
  assert.equal(logic.resourceCount(undefined), 0)
})

test("fixture links and absolute HTTP(S) URLs can open", () => {
  const saved = fixture("resources_list").find(row => row.kind === "link")
  assert.equal(logic.webUrl(saved.url), saved.url)
  assert.equal(logic.webUrl("  HTTP://localhost/path#one  "), "HTTP://localhost/path#one")
  assert.equal(logic.webUrl("https://bücher.example/"), "https://bücher.example/")
  assert.equal(logic.webUrl("http://[::1]:8080/"), "http://[::1]:8080/")
})

test("other schemes, missing hosts and malformed or ambiguous URLs are blocked", () => {
  const rejected = [
    null, "", "example.org", "//example.org", "file:///tmp/a", "ftp://example.org",
    "javascript:alert(1)", "data:text/html,hello", "https:example.org", "https:///path",
    "https://", "https://:443/path", "https://example.org:65536", "https://user@",
    "https://[invalid]", "https://\\example.org", "https://example.org/a b",
    "https://example.org/a\nb", "https://example.org/\u0000"
  ]
  for (const url of rejected) assert.equal(logic.webUrl(url), "", String(url))
})

test("link drafts upgrade bare domains to HTTPS and keep explicit web schemes", () => {
  const examples = [
    ["  youtube.com/watch?v=abc#start  ", "https://youtube.com/watch?v=abc#start"],
    ["example.com", "https://example.com"],
    ["example.com:8443/docs", "https://example.com:8443/docs"],
    ["bücher.example", "https://bücher.example"],
    ["http://example.com/docs", "http://example.com/docs"]
  ]
  for (const [value, expected] of examples) assert.equal(logic.normalizedWebUrl(value), expected)
})

test("link drafts reject relative paths, bare credentials, malformed domains and schemes", () => {
  for (const input of ["some text", "localhost", "docs/readme", "/example.org", "../example.org",
    "//example.org", "you@example.org", "example.org@other.org", "example.org:65536",
    "ftp://example.org", "mailto:you@example.org", "javascript:alert(1)", "https:example.org",
    "example..org", "-example.org", "example.org/a b", "example.org\\p",
    "example.org/a\nb", "example.org/\u0000", "http:///example.org"]) {
    assert.equal(logic.normalizedWebUrl(input), "", input)
  }
  assert.equal(logic.resourceDraft("link", "example.org/" + "a".repeat(2000)), null)
})

test("quick add sends the body or validated URL to the expanded item", () => {
  function same(actual, expected) {
    assert.deepEqual(JSON.parse(JSON.stringify(actual)), expected)
  }
  same(logic.quickResourceParams("item", "note", "  <b>Read</b>  "),
    { item_id: "item", kind: "note", body: "<b>Read</b>" })
  same(logic.quickResourceParams("item", "link", "  https://example.org/ "),
    { item_id: "item", kind: "link", url: "https://example.org/" })
  same(logic.quickResourceParams("item", "link", "example.org/video"),
    { item_id: "item", kind: "link", url: "https://example.org/video" })
  assert.equal(logic.quickResourceParams("item", "note", " "), null)
  assert.equal(logic.quickResourceParams("item", "link", "file:///tmp/example"), null)
  assert.equal(logic.quickResourceParams("item", "reminder", "tomorrow"), null)
})

test("task creation drafts are normalized without an item ID", () => {
  const note = logic.resourceDraft("note", "  Read\nnext chapter  ")
  const link = logic.resourceDraft("link", "example.org/guide")
  assert.deepEqual({ ...note }, { kind: "note", body: "Read\nnext chapter" })
  assert.deepEqual({ ...link }, { kind: "link", url: "https://example.org/guide" })
})

test("a reminder draft uses a local time and an optional title", () => {
  const expected = new Date(2028, 1, 29, 9, 15).toISOString()
  assert.deepEqual({ ...logic.resourceDraft("reminder", "  Plan March  ", "2028-02-29", "09:15") },
    { kind: "reminder", title: "Plan March", remind_at: expected })
  assert.deepEqual({ ...logic.resourceDraft("reminder", " ", "2028-02-29", "09:15") },
    { kind: "reminder", remind_at: expected })
  for (const [day, time] of [["2027-02-29", "09:15"], ["2028-02-30", "09:15"],
    ["2028-02-29", "24:00"], ["2028-02-29", ""], ["2028-02-29", "09:15Z"]]) {
    assert.equal(logic.resourceDraft("reminder", "Plan March", day, time), null)
  }
})
