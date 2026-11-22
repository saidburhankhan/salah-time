import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "."

Item {
    id: timeline
    property var win
    readonly property ListModel timelineModel: listModel
    property bool timelineSyncing: false
    property int timelineRevision: -1
    property bool dragging: false
    property string dragId: ""
    property int dragOriginalIndex: -1
    property real dragPointerY: 0
    property var dragPayload: ({})

    ListModel { id: listModel }

    function desiredTimelineRows() {
        var rows = [], blocks = backend.dayPlan.blocks || [], prayers = backend.dayPlan.prayers || []
        for (var b = 0; b < win.blockIds.length; b++) {
            var id = win.blockIds[b], prayer = b > 0 ? (prayers[b - 1] || {}) : ({})
            rows.push({
                key: "header:" + id,
                kind: "header",
                block: id,
                payload: {
                    name: win.blockNames[b],
                    time: b === 0 ? (backend.dayPlan.night_start || "--:--") : (prayer.time || "--:--"),
                    marked: !!prayer.marked,
                    current: id === (backend.dayPlan.now || {}).block
                }
            })
            var items = []
            for (var x = 0; x < blocks.length; x++)
                if (blocks[x].start === id) { items = blocks[x].items || []; break }
            for (var i = 0; i < items.length; i++)
                rows.push({ key: "item:" + items[i].id, kind: "item", block: id, payload: items[i] })
            rows.push({ key: "add:" + id, kind: "add", block: id, payload: {} })
            if (id === "fajr")
                rows.push({ key: "marker:sunrise", kind: "marker", block: id, payload: { name: qsTr("Sunrise"), time: backend.dayPlan.sunrise || "--:--" } })
            if (id === "isha")
                rows.push({ key: "marker:sleep", kind: "marker", block: id, payload: { name: qsTr("Sleep"), time: (backend.dayPlan.sleep || {}).time || "--:--" } })
        }
        return rows
    }

    function syncTimeline() {
        if (dragging || timelineRevision === backend.dayRevision)
            return
        timelineRevision = backend.dayRevision
        timelineSyncing = true
        var wanted = desiredTimelineRows()
        for (var i = 0; i < wanted.length; i++) {
            var found = -1
            for (var j = i; j < listModel.count; j++)
                if (listModel.get(j).key === wanted[i].key) { found = j; break }
            if (found < 0) {
                listModel.insert(i, wanted[i])
            } else {
                if (found !== i) listModel.move(found, i, 1)
                listModel.set(i, wanted[i])
            }
        }
        while (listModel.count > wanted.length)
            listModel.remove(listModel.count - 1)
        timelineSyncing = false
    }

    function beginDrag(rowIndex, id, globalY) {
        dragging = true
        dragId = id
        dragOriginalIndex = rowIndex
        dragPointerY = globalY
        dragPayload = listModel.get(rowIndex).payload || ({})
    }

    function updateDrag(globalY) {
        if (!dragging) return
        dragPointerY = globalY
        var p = todayList.mapFromItem(win.contentItem, 0, globalY)
        var idx = todayList.indexAt(10, p.y + todayList.contentY)
        if (idx < 0) idx = p.y < 0 ? 0 : listModel.count - 1
        var from = -1
        for (var i = 0; i < listModel.count; i++)
            if (listModel.get(i).key === "item:" + dragId) { from = i; break }
        // The gap belongs between a block's header and its "+ add" row: over a
        // header it goes just below it, over "+ add" just above it, and markers
        // (Sunrise, Sleep) never take it. Moving down, ListModel.move puts the
        // row after the target row; moving up, before it.
        var k = listModel.get(idx).kind
        if (k === "marker") return
        if (k === "header" && from > idx) idx++
        if (k === "add" && from < idx) idx--
        if (from >= 0 && idx !== from)
            listModel.move(from, Math.max(0, Math.min(idx, listModel.count - 1)), 1)
    }

    function finishDrag(cancelled) {
        if (!dragging) return
        var from = -1
        for (var i = 0; i < listModel.count; i++)
            if (listModel.get(i).key === "item:" + dragId) { from = i; break }
        if (cancelled || from < 0) {
            if (from >= 0 && dragOriginalIndex >= 0)
                listModel.move(from, dragOriginalIndex, 1)
        } else {
            // ListModel rows reach C++ as empty maps, so pass plain objects.
            var rows = []
            for (var r = 0; r < listModel.count; r++)
                if (r !== from) {
                    var e = listModel.get(r)
                    rows.push({ kind: e.kind, block: e.block })
                }
            var destination = backend.moveDestination(rows, from)
            if (destination.block)
                backend.moveItem(dragId, destination.block, destination.index)
        }
        dragging = false
        dragId = ""
        dragOriginalIndex = -1
        syncTimeline()
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
            required property string kind
            required property string block
            required property var payload
            required property string key
            width: todayList.width
            height: kind === "header" ? 60 : kind === "item" ? 82 : kind === "add" ? 32 : 26
            property var safePayload: payload || ({})
            PrayerHeader {
                anchors.fill: parent
                visible: rowDelegate.kind === "header"
                win: timeline.win
                block: rowDelegate.block
                payload: rowDelegate.safePayload
            }
            ItemCard {
                anchors.fill: parent
                visible: rowDelegate.kind === "item"
                win: timeline.win
                payload: rowDelegate.safePayload
                rowIndex: rowDelegate.index
                list: todayList
            }
            AddRow {
                anchors.fill: parent
                visible: rowDelegate.kind === "add"
                win: timeline.win
                block: rowDelegate.block
            }
            MarkerRow {
                visible: rowDelegate.kind === "marker"
                win: timeline.win
                payload: rowDelegate.safePayload
            }
        }
    }
}
