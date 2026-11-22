import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

RowLayout {
    id: stepper
    property var win
    property var payload: ({})
    spacing: 3
    RoundButton {
        text: "−"
        flat: true
        Layout.preferredWidth: 30
        Layout.preferredHeight: 30
        enabled: (stepper.payload.count || 0) > 0
        onClicked: backend.adjustCount(stepper.payload.id, -1)
    }
    Label {
        text: stepper.payload.target
            ? qsTr("%1 / %2").arg(stepper.payload.count || 0).arg(stepper.payload.target)
            : String(stepper.payload.count || 0)
        color: stepper.win.text
        font.family: stepper.win.mono
        font.pixelSize: 14
        horizontalAlignment: Text.AlignHCenter
        Layout.minimumWidth: stepper.payload.target ? 58 : 28
    }
    RoundButton {
        text: "+"
        flat: true
        Layout.preferredWidth: 30
        Layout.preferredHeight: 30
        onClicked: backend.adjustCount(stepper.payload.id, 1)
    }
}
