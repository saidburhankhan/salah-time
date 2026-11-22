import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: itemCard
    property var win
    property string itemId: ""
    property string title: ""
    property string state: ""
    property string measure: "check"
    property string repeat: "none"
    property string detailText: ""
    property bool pending: false
    property bool timerRunning: false
    property string timerStartedAt: ""
    property int timerMinutes: 0
    property bool timerProjected: false
    property int count: 0
    property int target: 0
    property string projectColor: "transparent"
    property int plannedMinutes: 0
    property int actualMinutes: 0
    property int sessions: 0
    property int rowIndex: 0
    property var list: null
    radius: theme.radius
    color: win.dragging && win.dragId === itemId ? "transparent" : "#1c1b18"
    border.color: win.dragging && win.dragId === itemId ? "#4a463e" : win.border
    opacity: pending ? 0.55 : 1

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 14
        spacing: 12
        opacity: itemCard.win.dragging && itemCard.win.dragId === itemCard.itemId ? 0 : 1
        Label {
            id: handle
            text: "⠿"
            color: itemCard.win.muted
            font.pixelSize: 19
            MouseArea {
                anchors.fill: parent
                anchors.margins: -8
                preventStealing: true
                onPressed: function(mouse) {
                    var p = mapToItem(itemCard.win.contentItem, mouse.x, mouse.y)
                    itemCard.win.beginDrag(itemCard.rowIndex, itemCard.itemId, p.y,
                        itemCard.title, itemCard.detailText, itemCard.state === "done")
                }
                onPositionChanged: function(mouse) {
                    if (pressed) {
                        var p = mapToItem(itemCard.win.contentItem, mouse.x, mouse.y)
                        itemCard.win.updateDrag(p.y)
                    }
                }
                onReleased: function(mouse) {
                    var p = mapToItem(itemCard.list, mouse.x, mouse.y)
                    itemCard.win.finishDrag(p.y < 0 || p.y > itemCard.list.height)
                }
                onCanceled: itemCard.win.finishDrag(true)
            }
        }
        Rectangle { width: 8; height: 8; radius: 4; color: itemCard.projectColor; visible: itemCard.projectColor !== "" }
        Rectangle {
            width: 24
            height: 24
            radius: Math.min(5, theme.radius)
            color: itemCard.state === "done" ? theme.accent : "transparent"
            border.color: itemCard.state === "done" ? theme.accent : "#625e55"
            Label {
                anchors.centerIn: parent
                text: "✓"
                visible: itemCard.state === "done"
                color: theme.accentForeground
            }
            MouseArea {
                anchors.fill: parent
                onClicked: backend.toggleDone(itemCard.itemId, itemCard.state)
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 4
            Label {
                text: itemCard.title
                color: itemCard.state === "done" ? itemCard.win.muted : itemCard.win.text
                font.pixelSize: 15
                font.weight: Font.Medium
                Layout.fillWidth: true
                elide: Text.ElideRight
                textFormat: Text.PlainText
            }
            Label {
                text: itemCard.detailText
                color: itemCard.win.muted
                font.family: itemCard.win.mono
                font.pixelSize: 12
            }
        }
        Label { textFormat: Text.PlainText; text: (itemCard.sessions === 1 ? qsTr("%1m · %2 session") : qsTr("%1m · %2 sessions")).arg(itemCard.plannedMinutes).arg(itemCard.sessions); color: itemCard.win.muted; font.family: itemCard.win.mono }
        Button {
            flat: true
            property int minutes: itemCard.win.timerMinutes(itemCard.timerMinutes,
                itemCard.timerStartedAt, itemCard.timerProjected, itemCard.timerRunning)
            text: itemCard.timerRunning ? qsTr("❚❚ %1m").arg(minutes) : qsTr("▶︎ %1m").arg(minutes)
            font.family: itemCard.win.mono
            onClicked: backend.toggleTimer(itemCard.itemId, itemCard.timerRunning)
        }
    }
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        onDoubleClicked: itemCard.win.openEntry(itemCard.itemId)
        z: -1
    }
}
