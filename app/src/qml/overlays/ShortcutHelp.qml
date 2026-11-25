import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Popup {
    id: helpPopup
    property var win
    visible: win ? win.helpVisible : false
    modal: true
    focus: true
    anchors.centerIn: Overlay.overlay
    width: 460
    height: 430
    closePolicy: Popup.NoAutoClose
    background: Rectangle {
        color: helpPopup.win.panel
        radius: theme.radius
        border.color: helpPopup.win.border
    }
    contentItem: ColumnLayout {
        function submitForm() { helpPopup.win.helpVisible = false }
        function cancelForm() { helpPopup.win.helpVisible = false }
        focus: true
        anchors.fill: parent
        anchors.margins: 26
        spacing: 12
        Label {
            text: qsTr("Keyboard shortcuts")
            color: helpPopup.win.text
            font.pixelSize: 20
            font.weight: Font.DemiBold
        }
        Repeater {
            model: [["Ctrl 1", qsTr("Today")], ["Ctrl 2", qsTr("Routines")],
                ["Ctrl 3", qsTr("Statistics")], ["Ctrl 4", qsTr("Settings")],
                ["Ctrl K", qsTr("Quick add")], ["Enter", qsTr("Save form")], ["Shift Enter", qsTr("New line in notes")],
                ["Esc", qsTr("Close top overlay / cancel drag")],
                ["← / →", qsTr("Previous / next day")], ["T", qsTr("Today")],
                ["?", qsTr("This help")], ["Ctrl Q / W", qsTr("Quit")],
                ["Q", qsTr("Quit outside text fields")]]
            RowLayout {
                required property var modelData
                Layout.fillWidth: true
                Label {
                    text: modelData[0]
                    color: theme.accent
                    font.family: helpPopup.win.mono
                    Layout.preferredWidth: 100
                }
                Label {
                    text: modelData[1]
                    color: helpPopup.win.text
                    Layout.fillWidth: true
                }
            }
        }
    }
}
