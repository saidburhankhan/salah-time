import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// The running session, wherever it was started (here, the bar widget, the
// CLI). Pause stops it without asking anything.
Card {
    id: runningCard
    objectName: "runningCard"
    property var win
    readonly property var session: backend.running
    readonly property bool active: !!(session && session.id)
    readonly property int elapsed: win.elapsedMinutes(session ? session.started_at : "")
    visible: active
    border.color: theme.accent

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 10

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Rectangle {
                width: 8
                height: 8
                radius: 4
                color: "#ef8068"
                SequentialAnimation on opacity {
                    running: runningCard.active
                    loops: Animation.Infinite
                    NumberAnimation { to: 0.25; duration: 900 }
                    NumberAnimation { to: 1; duration: 900 }
                }
            }
            Label {
                text: qsTr("RUNNING")
                color: runningCard.win.muted
                font.family: runningCard.win.mono
                font.pixelSize: 12
                font.letterSpacing: 1.5
            }
            Item {
                Layout.fillWidth: true
            }
            Label {
                text: qsTr("since %1").arg(runningCard.win.clockTime(runningCard.session ? runningCard.session.started_at : ""))
                color: runningCard.win.muted
                font.family: runningCard.win.mono
                font.pixelSize: 12
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            Rectangle {
                visible: backend.runningColor !== ""
                width: 12
                height: 12
                radius: 6
                color: backend.runningColor || "transparent"
            }
            Label {
                Layout.fillWidth: true
                text: backend.runningTitle
                textFormat: Text.PlainText
                color: runningCard.win.text
                font.pixelSize: 17
                font.weight: Font.Medium
                elide: Text.ElideRight
            }
        }
        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: runningCard.win.elapsedClock(runningCard.session ? runningCard.session.started_at : "")
                font.features: { "tnum": 1 }
                color: runningCard.win.text
                font.family: runningCard.win.mono
                font.pixelSize: 30
            }
            AppButton {
                objectName: "runningStopButton"
                win: runningCard.win
                primary: true
                text: qsTr("❚❚  Pause")
                onClicked: runningCard.win.stopRunning()
            }
        }
    }
}
