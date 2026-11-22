import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: correction
    property var win
    property var payload: ({})
    property string correctionError: ""
    spacing: 1
    Rectangle {
        Layout.preferredWidth: Math.max(88, Math.min(96, correctionInput.contentWidth + 28))
        Layout.preferredHeight: 36
        radius: 7
        color: "transparent"
        border.color: correction.correctionError ? "#ef8068" : correction.win.border
        TextField {
            id: correctionInput
            anchors.fill: parent
            anchors.margins: 1
            color: correction.win.text
            font.family: correction.win.mono
            font.pixelSize: 14
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            placeholderText: qsTr("± min")
            inputMethodHints: Qt.ImhFormattedNumbersOnly
            background: null
            padding: 2
            onAccepted: {
                var raw = text.trim(), n = Number(raw)
                if (!/^[+-]?\d+$/.test(raw) || n === 0 || n < -1440 || n > 1440) {
                    correction.correctionError = qsTr("Use ±1–1440")
                    correctionErrorTimer.restart()
                } else {
                    backend.adjustMinutes(correction.payload.id, n)
                    text = ""
                }
            }
            Keys.onEscapePressed: function(event) {
                text = ""
                correction.correctionError = ""
                focus = false
                event.accepted = true
            }
        }
    }
    Label {
        visible: correction.correctionError !== ""
        text: correction.correctionError
        color: "#ef8068"
        font.pixelSize: 9
        Layout.alignment: Qt.AlignHCenter
    }
    Timer {
        id: correctionErrorTimer
        interval: 2200
        onTriggered: correction.correctionError = ""
    }
}
