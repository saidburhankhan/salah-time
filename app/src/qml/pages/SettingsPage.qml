import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

Item {
    id: settingsPage
    property var win
    ColumnLayout {
        width: 620
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.margins: 34
        spacing: 18
        Card {
            win: settingsPage.win
            Layout.fillWidth: true
            height: 150
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 22
                Label {
                    text: qsTr("Sleep time")
                    color: settingsPage.win.text
                    font.pixelSize: 17
                    font.weight: Font.Medium
                }
                Label {
                    text: qsTr("The end of your Isha planning block.")
                    color: settingsPage.win.muted
                }
                RowLayout {
                    TextField {
                        id: sleepField
                        text: backend.config.sleepTime || "23:00"
                        placeholderText: "HH:MM"
                        font.family: settingsPage.win.mono
                        Layout.preferredWidth: 160
                    }
                    AppButton {
                        win: settingsPage.win
                        text: qsTr("Save")
                        primary: true
                        onClicked: backend.setSleepTime(sleepField.text)
                    }
                }
            }
        }
        Card {
            win: settingsPage.win
            Layout.fillWidth: true
            height: 180
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 22
                spacing: 12
                Label {
                    text: qsTr("Prayer calculation")
                    color: settingsPage.win.text
                    font.pixelSize: 17
                    font.weight: Font.Medium
                }
                RowLayout {
                    Layout.fillWidth: true
                    Label {
                        text: qsTr("Location")
                        color: settingsPage.win.muted
                        Layout.fillWidth: true
                    }
                    Label {
                        text: settingsPage.win.locationName()
                        color: settingsPage.win.text
                        font.family: settingsPage.win.mono
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Label {
                        text: qsTr("Method")
                        color: settingsPage.win.muted
                        Layout.fillWidth: true
                    }
                    Label {
                        text: settingsPage.win.methodName()
                        color: settingsPage.win.text
                        font.family: settingsPage.win.mono
                    }
                }
                Label {
                    text: qsTr("Change location and calculation method with the salah CLI.")
                    color: settingsPage.win.muted
                    font.pixelSize: 12
                }
            }
        }
    }
}
