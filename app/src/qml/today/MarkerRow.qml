import QtQuick
import QtQuick.Controls

Row {
    id: marker
    property var win
    property var payload: ({})
    height: 26
    spacing: 12
    leftPadding: 18
    Rectangle {
        width: 8
        height: 8
        radius: 4
        color: "#555148"
        anchors.verticalCenter: parent.verticalCenter
    }
    Label {
        text: (marker.payload.name || "") + "  " + (marker.payload.time || "--:--")
        color: marker.win.muted
        font.family: marker.win.mono
        font.pixelSize: 12
        anchors.verticalCenter: parent.verticalCenter
    }
}
