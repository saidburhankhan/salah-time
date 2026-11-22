import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: itemCard
    property var win
    property var payload: ({})
    property int rowIndex: 0
    property var list: null
    property var safePayload: payload || ({})
    radius: theme.radius
    color: win.dragging && win.dragId === safePayload.id ? "transparent" : "#1c1b18"
    border.color: win.dragging && win.dragId === safePayload.id ? "#4a463e" : win.border
    opacity: safePayload.pending ? 0.55 : 1

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 14
        spacing: 12
        opacity: itemCard.win.dragging && itemCard.win.dragId === itemCard.safePayload.id ? 0 : 1
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
                    itemCard.win.beginDrag(itemCard.rowIndex, itemCard.safePayload.id, p.y)
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
        Rectangle {
            width: 24
            height: 24
            radius: Math.min(5, theme.radius)
            color: itemCard.safePayload.state === "done" ? theme.accent : "transparent"
            border.color: itemCard.safePayload.state === "done" ? theme.accent : "#625e55"
            Label {
                anchors.centerIn: parent
                text: "✓"
                visible: itemCard.safePayload.state === "done"
                color: theme.accentForeground
            }
            MouseArea {
                anchors.fill: parent
                onClicked: backend.toggleDone(itemCard.safePayload.id, itemCard.safePayload.state)
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 4
            Label {
                text: itemCard.safePayload.title || ""
                color: itemCard.safePayload.state === "done" ? itemCard.win.muted : itemCard.win.text
                font.pixelSize: 15
                font.weight: Font.Medium
                Layout.fillWidth: true
                elide: Text.ElideRight
                textFormat: Text.PlainText
            }
            Label {
                text: itemCard.win.detail(itemCard.safePayload)
                color: itemCard.win.muted
                font.family: itemCard.win.mono
                font.pixelSize: 12
            }
        }
        CounterStepper {
            visible: itemCard.safePayload.measure === "count"
            win: itemCard.win
            payload: itemCard.safePayload
        }
        Button {
            flat: true
            text: itemCard.safePayload.timer_started_at
                ? qsTr("❚❚ %1m").arg(itemCard.win.runningMinutes(itemCard.safePayload))
                : qsTr("▶︎ %1m").arg(itemCard.win.runningMinutes(itemCard.safePayload))
            font.family: itemCard.win.mono
            onClicked: backend.toggleTimer(itemCard.safePayload.id, !!itemCard.safePayload.timer_started_at)
        }
        CorrectionBox {
            win: itemCard.win
            payload: itemCard.safePayload
        }
    }
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        onDoubleClicked: itemCard.win.openEntry(itemCard.safePayload)
        z: -1
    }
}
