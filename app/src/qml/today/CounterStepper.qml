import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

RowLayout {
    id: stepper
    property var win
    property string itemId: ""
    property int count: 0
    property int target: 0
    spacing: 3
    RoundButton {
        text: "−"
        flat: true
        Layout.preferredWidth: 30
        Layout.preferredHeight: 30
        enabled: stepper.count > 0
        onClicked: backend.adjustCount(stepper.itemId, -1)
    }
    Label {
        text: stepper.target
            ? qsTr("%1 / %2").arg(stepper.count).arg(stepper.target)
            : String(stepper.count)
        color: stepper.win.text
        font.family: stepper.win.mono
        font.pixelSize: 14
        horizontalAlignment: Text.AlignHCenter
        Layout.minimumWidth: stepper.target ? 58 : 28
    }
    RoundButton {
        text: "+"
        flat: true
        Layout.preferredWidth: 30
        Layout.preferredHeight: 30
        onClicked: backend.adjustCount(stepper.itemId, 1)
    }
}
