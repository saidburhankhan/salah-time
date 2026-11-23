import QtQuick
import QtQuick.Controls

// An active filter: its text opens it for editing, the × removes it. Same
// 40 px slot and 28 px face as ChoiceChip so the two line up in a Flow.
Item {
    id: chip
    property var win
    property string text: ""
    signal edit()
    signal remove()
    implicitWidth: label.implicitWidth + close.width + 22
    implicitHeight: 40
    width: implicitWidth
    height: implicitHeight

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        height: 28
        radius: Math.max(6, theme.radius)
        color: bodyArea.pressed ? chip.win.selectedFillPressed : chip.win.selectedFill
        border.width: 1
        border.color: chip.win.selectedBorder
    }

    Text {
        id: label
        anchors.left: parent.left
        anchors.leftMargin: 15
        anchors.verticalCenter: parent.verticalCenter
        text: chip.text
        textFormat: Text.PlainText
        color: chip.win.selectedText
        font.pixelSize: 14
        font.features: { "tnum": 1 }
    }
    MouseArea {
        id: bodyArea
        anchors.left: parent.left
        anchors.right: close.left
        height: parent.height
        cursorShape: Qt.PointingHandCursor
        onClicked: chip.edit()
    }
    Item {
        id: close
        objectName: "filterChipRemove"
        anchors.right: parent.right
        width: 28
        height: parent.height
        Text {
            anchors.centerIn: parent
            text: "×"
            color: closeArea.containsMouse ? chip.win.text : chip.win.selectedText
            font.pixelSize: 17
        }
        MouseArea {
            id: closeArea
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: chip.remove()
        }
    }
}
