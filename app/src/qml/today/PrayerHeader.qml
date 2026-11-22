import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: header
    property var win
    property string block: ""
    property string headerName: ""
    property string headerTime: ""
    property bool marked: false
    property bool current: false
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
            color: header.marked ? theme.accent : "transparent"
            border.width: 2
            border.color: header.marked ? theme.accent : header.win.text
            Label {
                anchors.centerIn: parent
                visible: header.marked
                text: "✓"
                color: theme.accentForeground
                font.weight: Font.Bold
            }
            MouseArea {
                anchors.fill: parent
                onClicked: backend.togglePrayer(header.block, header.marked)
            }
        }
        Label {
            text: header.headerName
            color: header.win.text
            font.pixelSize: 17
            font.weight: Font.Medium
            Layout.fillWidth: true
        }
        Label {
            text: header.headerTime || "--:--"
            color: "#d8d4cc"
            font.family: header.win.mono
            font.pixelSize: 17
        }
        Rectangle {
            visible: header.current
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
