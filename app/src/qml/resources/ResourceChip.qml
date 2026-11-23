import QtQuick
import QtQuick.Controls

// The paperclip chip on a task row: the resource count, opens the drawer.
Rectangle {
    id: chip
    required property var win
    property int count: 0
    property bool selected: false
    signal clicked()
    implicitHeight: 30
    implicitWidth: label.implicitWidth + 24
    radius: 15
    color: selected ? win.selectedFill : (area.containsMouse ? "#2c2a25" : "transparent")
    border.color: selected ? win.selectedBorder : win.border
    Label {
        id: label
        anchors.centerIn: parent
        text: "📎" + (chip.count > 0 ? " " + chip.count : "")
        color: chip.count > 0 ? chip.win.text : chip.win.muted
        font.pixelSize: 12
    }
    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: chip.clicked()
    }
    Accessible.role: Accessible.Button
    Accessible.name: chip.count === 1 ? qsTr("1 resource") : qsTr("%1 resources").arg(chip.count)
    ToolTip.visible: area.containsMouse
    ToolTip.delay: 500
    ToolTip.text: Accessible.name
}
