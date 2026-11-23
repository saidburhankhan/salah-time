import QtQuick
import QtQuick.Controls

Button {
    id: control
    property var win
    property bool selected: false
    // Optional colour dot before the label (projects, tags).
    property color dotColor: "transparent"
    readonly property bool hasDot: dotColor.a > 0
    implicitHeight: 40
    leftPadding: 15
    rightPadding: 15
    contentItem: Row {
        spacing: 8
        Rectangle {
            visible: control.hasDot
            width: 10
            height: 10
            radius: 5
            anchors.verticalCenter: parent.verticalCenter
            color: control.dotColor
            border.width: control.selected ? 1 : 0
            border.color: "#171612"
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: control.text
            textFormat: Text.PlainText
            color: !control.enabled ? "#6f6b63" : (control.selected ? "#171612" : "#d6d2ca")
            font.pixelSize: 14
        }
    }
    background: Rectangle {
        radius: Math.max(6, theme.radius)
        color: control.selected ? control.win.text : (control.down ? control.win.border : "transparent")
        border.width: 1
        border.color: control.selected ? control.win.text : control.win.border
        opacity: control.enabled ? 1 : 0.6
    }
}
