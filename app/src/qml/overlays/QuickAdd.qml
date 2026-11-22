import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Effects
import "../controls"

Popup {
    id: quickPopup
    property var win
    visible: win ? win.quickVisible : false
    modal: true
    focus: true
    anchors.centerIn: Overlay.overlay
    width: Math.min(760, win.width - 80)
    height: 245
    closePolicy: Popup.NoAutoClose
    Overlay.modal: Rectangle { color: "#8c000000" }
    background: Rectangle {
        color: "#1c1b18"
        radius: Math.max(8, theme.radius)
        border.width: 1
        border.color: quickPopup.win.border
        layer.enabled: true
        layer.effect: MultiEffect {
            shadowEnabled: true
            shadowOpacity: .4
            shadowBlur: 1
        }
    }
    onClosed: quickPopup.win.quickVisible = false
    function reset() {
        quickField.text = ""
        quickField.forceActiveFocus()
    }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 28
        spacing: 14
        RowLayout {
            Layout.fillWidth: true
            Label {
                text: quickPopup.win.quickBlock
                    ? qsTr("QUICK ADD · %1").arg(quickPopup.win.quickBlock.toUpperCase())
                    : qsTr("QUICK ADD")
                color: quickPopup.win.muted
                font.family: quickPopup.win.mono
                font.pixelSize: 12
                font.letterSpacing: 1.5
                Layout.fillWidth: true
            }
            Label {
                text: "Ctrl K"
                color: quickPopup.win.muted
                font.family: quickPopup.win.mono
            }
        }
        TextField {
            id: quickField
            Layout.fillWidth: true
            placeholderText: qsTr("e.g. read 20m after isha daily")
            font.pixelSize: 20
            onTextChanged: parseTimer.restart()
            onAccepted: if (text.trim()) {
                quickPopup.win.maybeToast({ title: backend.parseDraft.title || text })
                backend.addQuick(text, quickPopup.win.quickBlock)
                quickPopup.win.quickVisible = false
            }
        }
        Label {
            Layout.fillWidth: true
            text: backend.parseDraft.title
                ? qsTr("%1 · %2 · %3 %4")
                    .arg(backend.parseDraft.title)
                    .arg(backend.parseDraft.block || qsTr("next prayer"))
                    .arg(backend.parseDraft.hasPlannedMinutes ? backend.parseDraft.plannedMinutes : "")
                    .arg(backend.parseDraft.hasPlannedMinutes ? "min" : "")
                : qsTr("Describe what, when, and how often.")
            color: quickPopup.win.muted
            font.family: quickPopup.win.mono
            font.pixelSize: 12
            elide: Text.ElideRight
        }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            AppButton {
                win: quickPopup.win
                text: qsTr("Cancel")
                onClicked: quickPopup.win.quickVisible = false
            }
            AppButton {
                win: quickPopup.win
                text: qsTr("Edit details")
                onClicked: {
                    quickPopup.win.openEntryFromParse(quickField.text)
                    quickPopup.win.quickVisible = false
                }
            }
            AppButton {
                win: quickPopup.win
                text: qsTr("Add  ↵")
                primary: true
                enabled: quickField.text.trim() !== ""
                onClicked: {
                    quickPopup.win.maybeToast({ title: backend.parseDraft.title || quickField.text })
                    backend.addQuick(quickField.text, quickPopup.win.quickBlock)
                    quickPopup.win.quickVisible = false
                }
            }
        }
    }
    Timer {
        id: parseTimer
        interval: 200
        onTriggered: backend.parseQuick(quickField.text)
    }
}
