import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

Rectangle {
    id: row
    required property var win
    required property var task
    readonly property var drag: win.backlogDrag
    readonly property int resourceCount: {
        var counts = task.resources || {}
        return (counts.notes || 0) + (counts.links || 0) + (counts.reminders || 0)
    }
    readonly property bool dragged: drag.active && drag.task.id === task.id
    readonly property bool dropTarget: drag.active && drag.anchorId === task.id

    implicitHeight: 55
    height: implicitHeight
    radius: theme.radius
    color: win.card
    border.color: win.border

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        spacing: 12
        opacity: row.dragged ? 0.35 : 1

        Label {
            text: "⠿"
            color: row.win.muted
            font.pixelSize: 19
            Layout.preferredWidth: 22
            MouseArea {
                id: handle
                objectName: "backlogHandle_" + row.task.id
                anchors.fill: parent
                anchors.margins: -5
                enabled: !backend.offlineMode
                cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                hoverEnabled: true
                preventStealing: true
                property point pressPoint
                onPressed: function(mouse) {
                    pressPoint = Qt.point(mouse.x, mouse.y)
                }
                onPositionChanged: function(mouse) {
                    if (!pressed)
                        return
                    var point = mapToItem(row.win.contentItem, mouse.x, mouse.y)
                    if (!row.drag.active && Math.hypot(mouse.x - pressPoint.x, mouse.y - pressPoint.y)
                            >= Qt.styleHints.startDragDistance)
                        row.drag.begin(row.task, point)
                    if (row.drag.active)
                        row.drag.move(point)
                }
                onReleased: function(mouse) {
                    if (row.drag.active) {
                        row.drag.move(mapToItem(row.win.contentItem, mouse.x, mouse.y))
                        row.drag.finish(false)
                    }
                }
                onCanceled: if (row.drag.active) row.drag.finish(true)
            }
            ToolTip.visible: handle.containsMouse && !handle.pressed
            ToolTip.delay: 500
            ToolTip.text: qsTr("Drag to reorder, or drag to Today and drop on a prayer block.")
        }
        Label {
            Layout.fillWidth: true
            text: row.task.title
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: row.win.text
            font.pixelSize: 15
            font.weight: Font.Medium
        }
        Label {
            Layout.preferredWidth: 85
            text: row.task.planned_minutes != null ? qsTr("%1 min").arg(row.task.planned_minutes) : qsTr("—")
            textFormat: Text.PlainText
            color: row.win.muted
            font.family: row.win.mono
            horizontalAlignment: Text.AlignRight
            Accessible.name: row.task.planned_minutes != null ? qsTr("%1 planned minutes").arg(row.task.planned_minutes)
                : qsTr("No planned minutes")
        }
        Label {
            Layout.preferredWidth: 92
            text: row.resourceCount === 1 ? qsTr("1 resource") : qsTr("%1 resources").arg(row.resourceCount)
            textFormat: Text.PlainText
            color: row.win.muted
            font.pixelSize: 12
            horizontalAlignment: Text.AlignRight
        }
        AppButton {
            objectName: "backlogEdit_" + row.task.id
            win: row.win
            text: qsTr("Edit")
            implicitHeight: 34
            enabled: !backend.offlineMode
            onClicked: row.win.openEntry(row.task.id)
        }
        AppButton {
            objectName: "backlogSchedule_" + row.task.id
            win: row.win
            text: qsTr("Schedule")
            primary: true
            implicitHeight: 34
            enabled: !backend.offlineMode
            onClicked: row.win.openSchedule(row.task)
        }
    }

    Rectangle {
        visible: row.dropTarget
        anchors.left: parent.left
        anchors.right: parent.right
        y: row.drag.before ? 0 : parent.height - height
        height: 2
        color: row.win.selectedBorder
    }
}
