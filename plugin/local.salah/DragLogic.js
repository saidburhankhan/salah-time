.pragma library

// Pure timeline drag rules shared by the bar widget. The list always looks
// like: header(block), item..., add(block), [marker...]. The only valid gap is
// between a block header and that block's add row. Markers never take a gap.
//
// targetIndex() returns the index to pass to ListModel.move(from, target, 1),
// or the current index when the drag should not move. destination() inspects
// the final visual order and returns the server move destination.

function kindOf(row) {
  return String(row && (row.rowType || row.kind) || "")
}

function keyOf(row) {
  return String(row && (row.rowKey || row.key) || "")
}

function blockOf(row) {
  return String(row && row.block || "")
}

function itemIdOf(row) {
  return String(row && (row.itemId || row.id) || "")
}

function indexOfRow(rows, key) {
  for (var i = 0; i < rows.length; i++)
    if (keyOf(rows[i]) === String(key || "")) return i
  return -1
}

// The gap at the end of the last block, i.e. just above its add row. This is
// where dragging below the whole list clamps, including below Sleep.
function lastBlockEndIndex(rows, from) {
  var addIndex = -1
  for (var i = rows.length - 1; i >= 0; i--)
    if (kindOf(rows[i]) === "add") { addIndex = i; break }
  if (addIndex < 0) return from
  return addIndex - (from < addIndex ? 1 : 0)
}

function targetIndex(rows, dragKey, hitIndex, afterHit, outside) {
  var from = indexOfRow(rows, dragKey)
  if (from < 0) return -1
  var count = rows.length
  outside = String(outside || "")

  if (outside === "below" || hitIndex >= count) return lastBlockEndIndex(rows, from)
  if (outside === "above" || hitIndex < 0) return from > 1 ? 1 : from

  var hit = rows[hitIndex]
  var kind = kindOf(hit)
  // Sunrise and Sleep never take the gap: keep the previous valid position.
  if (kind === "marker") return from
  // Over a header the gap is just below it.
  if (kind === "header") return hitIndex + (from > hitIndex ? 1 : 0)
  // Over an add row the gap is just above it.
  if (kind === "add") return hitIndex - (from < hitIndex ? 1 : 0)
  // Over an item: before it above the midpoint, after it below the midpoint.
  if (kind === "item") {
    var slot = hitIndex + (afterHit === true ? 1 : 0)
    return slot > from ? slot - 1 : slot
  }
  return from
}

function invalidDestination() {
  return {
    "valid": false, "block": "", "index": 0,
    "after_id": "", "before_id": "", "gap": -1
  }
}

// Remove the dragged row and describe the gap it leaves: block, index inside
// that block, and the neighbouring item ids. Invalid whenever the gap is not
// between a block header and that block's own add row (for example after the
// last add row or onto a marker).
function destination(rows, dragKey) {
  var from = indexOfRow(rows, dragKey)
  if (from < 0 || kindOf(rows[from]) !== "item") return invalidDestination()

  var rest = []
  for (var i = 0; i < rows.length; i++)
    if (i !== from) rest.push(rows[i])

  var gap = from
  var block = ""
  var index = 0
  var afterId = ""
  for (var before = 0; before < gap; before++) {
    var kindBefore = kindOf(rest[before])
    if (kindBefore === "header") {
      block = blockOf(rest[before])
      index = 0
      afterId = ""
    } else if (kindBefore === "item" && blockOf(rest[before]) === block) {
      index++
      afterId = itemIdOf(rest[before])
    }
  }

  var addIndex = -1
  var beforeId = ""
  for (var after = gap; after < rest.length; after++) {
    var kindAfter = kindOf(rest[after])
    if (kindAfter === "header") break
    if (kindAfter === "add" && blockOf(rest[after]) === block) {
      addIndex = after
      break
    }
    if (kindAfter === "item" && blockOf(rest[after]) === block && beforeId === "")
      beforeId = itemIdOf(rest[after])
  }

  if (block === "" || addIndex < 0) return invalidDestination()

  return {
    "valid": true, "block": block, "index": index,
    "after_id": afterId, "before_id": beforeId, "gap": gap
  }
}
