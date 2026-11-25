# Interaction spec: drag and drop and responsiveness

Shared by the bar widget and the desktop app. The widget is the most used
surface, so it must feel at least as fast and direct as the app.

## Drag and drop (reorderable list, not a free board)

The timeline is one flat, ordered list: for each block, a header row (the
prayer, or "Before Fajr" for the night block), its item rows, then its
"+ add" row; Sunrise and Sleep are passive marker rows.

- Grab: press on the item's drag handle (or long-press ~250 ms anywhere on the
  item row in the widget, where rows are small). The item lifts: slightly
  raised (shadow or scale 1.02), follows the pointer vertically only.
- While dragging, the other item rows move out of the way with a short
  animation (~120 ms) so a gap of the dragged row's height shows exactly where
  it will land. The gap can move into any block, including empty blocks
  (right under the header) and the night block.
- Drop: the row settles into the gap. The new block is the nearest block
  header above the gap; the index is the number of items between that header
  and the gap. Then run `salah item move --json ID --block B --index N`.
- Escape or dropping outside the list cancels: the row animates back.
- Never leave a row where the pointer released it; the list always shows the
  model order.
- Implementation guidance (QtQuick): keep a ListModel of rows; on drag-over,
  call `model.move(from, to, 1)` so rows shift live (`ListView` with `move`/
  `displaced` transitions, or a `DelegateModel` + `DropArea` per row). Keep
  the dragged delegate's `Drag.active` item separate from the layout (do not
  bind `drag.target` to a layout-managed delegate), and reset it on release.
- Optimistic: the reordered list is what the user sees immediately; the CLI
  call runs in the background; on failure, show the error and restore the
  previous order from the next refresh.

## Responsiveness

The CLI answers in about 2 ms; perceived lag comes from the UI.

- Optimistic updates for every action: tick/untick, prayer mark/unmark, +1,
  timer start/stop, move, add (show the new row immediately as pending,
  muted, until the refresh confirms it). Then refresh once after the queue is
  idle, not after every single action.
- Do not rebuild delegates on refresh: compare the new JSON text with the
  previous one and skip the update when unchanged; when changed, update the
  row ListModel in place (set changed roles, insert/remove/move rows) so
  delegates, scroll position, focus, and hover survive.
- No work in tight loops: the 1-second countdown updates only the countdown
  text, never the models.
- Keep animations short (≤150 ms) and only on things that move.
