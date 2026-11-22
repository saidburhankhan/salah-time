import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "."

Item {
    id: timeline
    property var win
    readonly property var timelineModel: backend.dayTimeline
    property bool dragging: false
    property string dragId: ""
    property int dragOriginalIndex: -1
    property real dragPointerY: 0
    property string dragTitle: ""
    property string dragDetail: ""
    property bool dragDone: false
    // Compatibility for callers that used the old payload map; the ghost
    // itself binds the three typed properties above.
    readonly property var dragPayload: ({ title: dragTitle, detail_text: dragDetail,
                                          state: dragDone ? "done" : "open" })

    // The model is owned and diffed by Backend now; kept for callers that used
    // to force a QML-side ListModel sync.
    function syncTimeline() {}

    function beginDrag(rowIndex, id, globalY, title, detail, done) {
        dragging = true
        dragId = id
        dragOriginalIndex = rowIndex
        dragPointerY = globalY
        dragTitle = title || ""
        dragDetail = detail || ""
        dragDone = !!done
    }

    function updateDrag(globalY) {
        if (!dragging) return
        dragPointerY = globalY
        var model = backend.dayTimeline
        var p = todayList.mapFromItem(win.contentItem, 0, globalY)
        var contentY = p.y + todayList.contentY
        var idx = todayList.indexAt(10, contentY)
        var from = model.indexOfItem(dragId)
        // Above the list clamps below the first header; below the content
        // clamps to the gap just above the last block's "+ add" row, never
        // after the Sleep marker. Points between rows keep the current gap.
        if (idx < 0) {
            if (contentY < 0) {
                idx = 0
            } else if (contentY >= todayList.contentHeight) {
                idx = model.count - 1
                while (idx >= 0 && model.kindAt(idx) !== "add") idx--
                if (idx < 0) return
            } else {
                return
            }
        }
        // The gap belongs between a block's header and its "+ add" row: over a
        // header it goes just below it, over "+ add" just above it, and markers
        // (Sunrise, Sleep) never take it. Moving down, moveRow puts the row
        // after the target row; moving up, before it.
        var kind = model.kindAt(idx)
        if (kind === "marker") return
        if (kind === "header" && from > idx) idx++
        if (kind === "add" && from < idx) idx--
        if (from >= 0 && idx !== from)
            model.moveRow(from, Math.max(0, Math.min(idx, model.count - 1)))
    }

    function finishDrag(cancelled) {
        if (!dragging) return
        var model = backend.dayTimeline
        var from = model.indexOfItem(dragId)
        if (cancelled || from < 0) {
            if (from >= 0 && dragOriginalIndex >= 0)
                model.moveRow(from, dragOriginalIndex)
        } else {
            // The dragged row keeps its old block role while the list is
            // reordered, so resolve the destination from the nearest header
            // above the gap instead of from model.blockAt(from).
            var block = ""
            for (var h = from; h >= 0; h--) {
                if (model.kindAt(h) === "header") { block = model.blockAt(h); break }
            }
            if (block === "") {
                if (dragOriginalIndex >= 0)
                    model.moveRow(from, dragOriginalIndex)
                dragging = false
                dragId = ""
                dragOriginalIndex = -1
                return
            }
            var afterId = "", beforeId = ""
            for (var p = from - 1; p >= 0; p--) {
                if (model.kindAt(p) === "header") break
                if (model.kindAt(p) === "item" && model.blockAt(p) === block) {
                    afterId = model.itemIdAt(p)
                    break
                }
            }
            for (var n = from + 1; n < model.count; n++) {
                var kind = model.kindAt(n)
                if (kind === "add" || kind === "header" || kind === "marker") break
                if (kind === "item" && model.blockAt(n) === block) {
                    beforeId = model.itemIdAt(n)
                    break
                }
            }
            backend.moveItem(dragId, block, afterId, beforeId)
        }
        dragging = false
        dragId = ""
        dragOriginalIndex = -1
    }

    ListView {
        id: todayList
        anchors.fill: parent
        clip: true
        spacing: 8
        model: timeline.timelineModel
        move: Transition {
            NumberAnimation { properties: "y"; duration: 120; easing.type: Easing.OutCubic }
        }
        displaced: Transition {
            NumberAnimation { properties: "y"; duration: 120; easing.type: Easing.OutCubic }
        }
        delegate: Item {
            id: rowDelegate
            required property int index
            required property string rowKind
            required property string block
            required property string name
            required property string time
            required property bool marked
            required property bool current
            required property string itemId
            required property string title
            required property string state
            required property string measure
            required property string repeat
            required property string detailText
            required property bool pending
            required property bool timerRunning
            required property string timerStartedAt
            required property int timerMinutes
            required property bool timerProjected
            required property int count
            required property int target
            width: todayList.width
            height: rowKind === "header" ? 60 : rowKind === "item" ? 82 : rowKind === "add" ? 32 : 26
            PrayerHeader {
                anchors.fill: parent
                visible: rowDelegate.rowKind === "header"
                win: timeline.win
                block: rowDelegate.block
                headerName: rowDelegate.name
                headerTime: rowDelegate.time
                marked: rowDelegate.marked
                current: rowDelegate.current
            }
            ItemCard {
                anchors.fill: parent
                visible: rowDelegate.rowKind === "item"
                win: timeline.win
                itemId: rowDelegate.itemId
                title: rowDelegate.title
                state: rowDelegate.state
                measure: rowDelegate.measure
                repeat: rowDelegate.repeat
                detailText: rowDelegate.detailText
                pending: rowDelegate.pending
                timerRunning: rowDelegate.timerRunning
                timerStartedAt: rowDelegate.timerStartedAt
                timerMinutes: rowDelegate.timerMinutes
                timerProjected: rowDelegate.timerProjected
                count: rowDelegate.count
                target: rowDelegate.target
                rowIndex: rowDelegate.index
                list: todayList
            }
            AddRow {
                anchors.fill: parent
                visible: rowDelegate.rowKind === "add"
                win: timeline.win
                block: rowDelegate.block
            }
            MarkerRow {
                visible: rowDelegate.rowKind === "marker"
                win: timeline.win
                markerName: rowDelegate.name
                markerTime: rowDelegate.time
            }
        }
    }
}
