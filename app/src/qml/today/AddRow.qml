import QtQuick
import QtQuick.Controls

Rectangle {
    id: addRow
    property var win
    property string block: ""
    color: "transparent"
    Label {
        anchors.left: parent.left
        anchors.leftMargin: 56
        anchors.verticalCenter: parent.verticalCenter
        text: qsTr("+  add")
        color: "#777269"
        font.pixelSize: 13
    }
    MouseArea {
        anchors.fill: parent
        onClicked: addRow.win.openQuick(addRow.block)
    }
}
