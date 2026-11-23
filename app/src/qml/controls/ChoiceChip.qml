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
    contentItem: Item {
        implicitWidth: chipRow.implicitWidth
        implicitHeight: chipRow.implicitHeight
        Row {
            id: chipRow
            anchors.centerIn: parent
            spacing: 8
            Rectangle {
                visible: control.hasDot
                width: 10
                height: 10
                radius: 5
                anchors.verticalCenter: parent.verticalCenter
                color: control.dotColor
                border.width: 0
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: control.text
                textFormat: Text.PlainText
                color: !control.enabled ? "#6f6b63" : (control.selected ? control.win.selectedText : "#d6d2ca")
                font.pixelSize: 14
            }
        }
    }
    background: Rectangle {
        radius: Math.max(6, theme.radius)
        color: control.selected
            ? (control.down ? control.win.selectedFillPressed : control.win.selectedFill)
            : (control.down ? control.win.border : "transparent")
        border.width: 1
        border.color: control.selected ? control.win.selectedBorder : control.win.border
        opacity: control.enabled ? 1 : 0.6
    }
}
