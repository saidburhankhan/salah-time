import QtQuick
import QtQuick.Controls

Button {
    id: control
    property var win
    property bool selected: false
    implicitHeight: 40
    leftPadding: 15
    rightPadding: 15
    contentItem: Text {
        text: control.text
        color: control.selected ? "#171612" : "#d6d2ca"
        font.pixelSize: 14
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }
    background: Rectangle {
        radius: Math.max(6, theme.radius)
        color: control.selected ? control.win.text : (control.down ? control.win.border : "transparent")
        border.width: 1
        border.color: control.selected ? control.win.text : control.win.border
    }
}
