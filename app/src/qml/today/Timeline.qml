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
    // Items whose session list is open, by id.
    property var expanded: ({})
    function toggleExpanded(id) {
        var copy = {}
        for (var key in expanded)
            copy[key] = expanded[key]
        if (copy[id])
            delete copy[id]
        else
            copy[id] = true
        expanded = copy
    }
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
            // Prefer the following sibling when both sides of the gap are
            // known. This preserves the user's exact "drop before" intent;
            // CoreClient deliberately serializes only one anchor.
            if (beforeId !== "") afterId = ""
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
        header: Item {
            width: todayList.width
            height: 72
            Rectangle {
                width: parent.width
                height: 64
                radius: theme.radius
                color: timeline.win.card
                border.color: timeline.win.border
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 18
                    anchors.rightMargin: 18
                    spacing: 18
                    Label {
                        text: qsTr("Today fulfilment")
                        color: timeline.win.text
                        font.weight: Font.DemiBold
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    Label {
                        text: qsTr("%1 / %2 items").arg(backend.doneItems).arg(backend.plannedItems)
                        color: timeline.win.muted
                    }
                    Label {
                        text: qsTr("%1 / %2 min").arg(backend.actualMinutes).arg(backend.plannedMinutes)
                        color: timeline.win.text
                        font.family: timeline.win.mono
                    }
                    Label {
                        text: qsTr("+%1 unplanned").arg(backend.unplannedMinutes)
                        visible: backend.unplannedMinutes > 0
                        color: timeline.win.muted
                        font.family: timeline.win.mono
                    }
                    Label {
                        text: backend.fulfilmentRatio == null ? qsTr("—")
                            : qsTr("%1%").arg(Math.round(Number(backend.fulfilmentRatio) * 100))
                        color: theme.accent
                        font.family: timeline.win.mono
                        font.pixelSize: 16
                    }
                }
            }
        }
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
            required property string projectColor
            required property int plannedMinutes
            required property int actualMinutes
            required property int sessions
            required property int capacityMinutes
            required property int blockPlannedMinutes
            required property int blockActualMinutes
            required property string projectId
            required property string projectName
            required property var sessionList
            required property bool ticked
            required property string runningSessionId
            readonly property bool open: rowKind === "item" && sessions > 0 && !!timeline.expanded[itemId]
            width: todayList.width
            height: rowKind === "header" ? 60
                : rowKind === "item" ? 82 + (open ? sessions * 52 + 8 : 0)
                : rowKind === "add" ? 32 : 26
            PrayerHeader {
                anchors.fill: parent
                visible: rowDelegate.rowKind === "header"
                win: timeline.win
                block: rowDelegate.block
                headerName: rowDelegate.name
                headerTime: rowDelegate.time
                marked: rowDelegate.marked
                current: rowDelegate.current
                capacityMinutes: rowDelegate.capacityMinutes
                plannedMinutes: rowDelegate.blockPlannedMinutes
                actualMinutes: rowDelegate.blockActualMinutes
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
                projectColor: rowDelegate.projectColor
                plannedMinutes: rowDelegate.plannedMinutes
                actualMinutes: rowDelegate.actualMinutes
                sessions: rowDelegate.sessions
                projectId: rowDelegate.projectId
                projectName: rowDelegate.projectName
                sessionList: rowDelegate.sessionList
                ticked: rowDelegate.ticked
                runningSessionId: rowDelegate.runningSessionId
                expanded: rowDelegate.open
                onToggleExpanded: timeline.toggleExpanded(rowDelegate.itemId)
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
