import QtQuick
import QtQuick.Controls
import "FormKeys.js" as FormKeys

Button {
    id: control
    property var win
    Keys.onPressed: event => FormKeys.route(event, control.win)
    property bool primary: false
    implicitHeight: 42
    leftPadding: 17
    rightPadding: 17
    contentItem: Text {
        text: control.text
        color: control.enabled ? (control.primary ? control.win.selectedText : control.win.text) : control.win.muted
        font.pixelSize: 14
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }
    background: Rectangle {
        radius: Math.max(6, theme.radius)
        color: control.primary
            ? (!control.enabled ? "#24221e" : (control.down ? control.win.selectedFillPressed : control.win.selectedFill))
            : (control.down ? control.win.border : "transparent")
        border.width: 1
        border.color: control.primary && control.enabled ? control.win.selectedBorder : control.win.border
    }
}
