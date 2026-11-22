import QtQuick
import QtQuick.Controls

Row {
    id: marker
    property var win
    property string markerName: ""
    property string markerTime: ""
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
        text: marker.markerName + "  " + (marker.markerTime || "--:--")
        color: marker.win.muted
        font.family: marker.win.mono
        font.pixelSize: 12
        anchors.verticalCenter: parent.verticalCenter
    }
}
