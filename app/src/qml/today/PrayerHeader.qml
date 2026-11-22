import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: header
    property var win
    property string block: ""
    property var payload: ({})
    property var safePayload: payload || ({})
    radius: theme.radius
    color: win.card
    border.color: win.border
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 18
        anchors.rightMargin: 16
        spacing: 14
        Rectangle {
            visible: header.block !== "night"
            width: 28
            height: 28
            radius: 14
            color: header.safePayload.marked ? theme.accent : "transparent"
            border.width: 2
            border.color: header.safePayload.marked ? theme.accent : header.win.text
            Label {
                anchors.centerIn: parent
                visible: !!header.safePayload.marked
                text: "✓"
                color: theme.accentForeground
                font.weight: Font.Bold
            }
            MouseArea {
                anchors.fill: parent
                onClicked: backend.togglePrayer(header.block, header.safePayload.marked || false)
            }
        }
        Label {
            text: header.safePayload.name || ""
            color: header.win.text
            font.pixelSize: 17
            font.weight: Font.Medium
            Layout.fillWidth: true
        }
        Label {
            text: header.safePayload.time || "--:--"
            color: "#d8d4cc"
            font.family: header.win.mono
            font.pixelSize: 17
        }
        Rectangle {
            visible: header.safePayload.current || false
            width: 62
            height: 28
            radius: 14
            color: "#4a2d26"
            Label {
                anchors.centerIn: parent
                text: qsTr("● NOW")
                color: "#ef8068"
                font.family: header.win.mono
                font.pixelSize: 11
            }
        }
    }
}
