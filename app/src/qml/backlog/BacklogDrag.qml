import QtQuick
import QtQuick.Controls

Item {
    id: drag
    required property var win
    required property var backlogPage
    required property var todayPage
    required property var todayLink
    property var task: null
    readonly property bool active: task !== null
    property point pointer
    property string anchorId: ""
    property bool before: false
    property string block: ""
    property bool overToday: false

    anchors.fill: parent
    visible: active
    z: 1100

    function begin(item, point) {
        backlogPage.dragRows = backlogPage.liveRows
        pointer = point
        task = item
    }

    function move(point) {
        if (!active)
            return
        pointer = point
        if (win.page === 5 && todayLink.contains(todayLink.mapFromItem(win.contentItem, point))) {
            overToday = true
            win.page = 0
            if (backend.selectedDay !== backend.todayDay)
                backend.jumpToday()
        }
        updateTarget()
    }

    function updateTarget() {
        var target = !overToday ? backlogPage.reorderTarget(pointer, task) : null
        anchorId = target ? target.id : ""
        before = target ? target.before : false
        block = overToday && backend.selectedDay === backend.todayDay ? todayPage.dropBlock(pointer) : ""
    }

    function finish(cancelled) {
        if (!active)
            return
        var id = task.id
        var destinationBlock = cancelled ? "" : block
        var destinationAnchor = cancelled ? "" : anchorId
        var insertBefore = before
        var returnToBacklog = overToday && destinationBlock === ""
        task = null
        anchorId = ""
        block = ""
        overToday = false
        if (destinationBlock)
            backend.scheduleItem(id, backend.todayDay, destinationBlock)
        else if (destinationAnchor)
            backend.moveBacklogItem(id, destinationAnchor, insertBefore)
        else if (returnToBacklog)
            win.page = 5
    }

    Timer {
        running: drag.active
        interval: 45
        repeat: true
        onTriggered: {
            if (drag.overToday)
                drag.todayPage.scrollForBacklogDrag(drag.pointer)
            else
                drag.backlogPage.scrollForDrag(drag.pointer)
            drag.updateTarget()
        }
    }

    Rectangle {
        x: Math.max(6, Math.min(drag.pointer.x + 16, drag.width - width - 8))
        y: Math.max(6, Math.min(drag.pointer.y + 14, drag.height - height - 8))
        width: 248
        height: 54
        radius: theme.radius
        color: "#292722"
        border.color: drag.block ? drag.win.selectedBorder : drag.win.border

        Column {
            anchors.fill: parent
            anchors.margins: 9
            spacing: 2
            Label {
                width: parent.width
                text: drag.task ? drag.task.title : ""
                textFormat: Text.PlainText
                color: drag.win.text
                elide: Text.ElideRight
                font.weight: Font.DemiBold
            }
            Label {
                width: parent.width
                text: drag.block ? qsTr("Today · %1").arg(drag.win.blockNames[drag.win.blockIds.indexOf(drag.block)])
                    : drag.overToday ? qsTr("Drop on a prayer block") : qsTr("Reorder here or drag to Today")
                textFormat: Text.PlainText
                color: drag.win.muted
                elide: Text.ElideRight
                font.pixelSize: 12
            }
        }
    }
}
