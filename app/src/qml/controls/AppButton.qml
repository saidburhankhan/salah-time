import QtQuick
import QtQuick.Controls

Button {
    id: control
    property var win
    property bool primary: false
    implicitHeight: 42
    leftPadding: 17
    rightPadding: 17
    contentItem: Text {
        text: control.text
        color: control.enabled ? (control.primary ? "#171612" : control.win.text) : control.win.muted
        font.pixelSize: 14
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }
    background: Rectangle {
        radius: Math.max(6, theme.radius)
        color: control.primary
            ? (control.enabled ? control.win.text : "#2c2a25")
            : (control.down ? control.win.border : "transparent")
        border.width: control.primary ? 0 : 1
        border.color: control.win.border
    }
}
