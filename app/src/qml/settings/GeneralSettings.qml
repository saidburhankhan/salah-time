import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

Item {
    id: general
    property var win

    ColumnLayout {
        width: Math.min(620, parent.width)
        anchors.top: parent.top
        anchors.left: parent.left
        spacing: 18

        Card {
            win: general.win
            Layout.fillWidth: true
            Layout.preferredHeight: sleepColumn.implicitHeight + 44
            ColumnLayout {
                id: sleepColumn
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 22
                spacing: 6
                Label {
                    text: qsTr("Sleep time")
                    color: general.win.text
                    font.pixelSize: 17
                    font.weight: Font.Medium
                }
                Label {
                    text: qsTr("The end of your Isha planning block.")
                    color: general.win.muted
                }
                RowLayout {
                    Layout.topMargin: 10
                    spacing: 10
                    TextField {
                        id: sleepField
                        text: backend.config.sleepTime || "23:00"
                        placeholderText: "HH:MM"
                        font.family: general.win.mono
                        Layout.preferredWidth: 160
                    }
                    AppButton {
                        win: general.win
                        text: qsTr("Save")
                        primary: true
                        onClicked: backend.setSleepTime(sleepField.text)
                    }
                }
            }
        }

        Card {
            win: general.win
            Layout.fillWidth: true
            Layout.preferredHeight: prayerColumn.implicitHeight + 44
            ColumnLayout {
                id: prayerColumn
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 22
                spacing: 12
                Label {
                    text: qsTr("Prayer calculation")
                    color: general.win.text
                    font.pixelSize: 17
                    font.weight: Font.Medium
                }
                RowLayout {
                    Layout.fillWidth: true
                    Label {
                        text: qsTr("Location")
                        color: general.win.muted
                        Layout.fillWidth: true
                    }
                    Label {
                        text: general.win.locationName()
                        textFormat: Text.PlainText
                        color: general.win.text
                        font.family: general.win.mono
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Label {
                        text: qsTr("Method")
                        color: general.win.muted
                        Layout.fillWidth: true
                    }
                    Label {
                        text: general.win.methodName()
                        textFormat: Text.PlainText
                        color: general.win.text
                        font.family: general.win.mono
                    }
                }
                Repeater {
                    model: [
                        { name: qsTr("Asr"), value: general.win.capitalized(backend.config.asr) || "—" },
                        { name: qsTr("Coordinates"),
                          value: backend.config.latitude || backend.config.longitude
                              ? Number(backend.config.latitude).toFixed(3) + ", " + Number(backend.config.longitude).toFixed(3)
                              : "—" },
                        { name: qsTr("Location source"),
                          value: backend.config.autoLocation
                              ? qsTr("Automatic") + (backend.config.lastLocationProvider ? " · " + backend.config.lastLocationProvider : "")
                              : qsTr("Manual") }
                    ]
                    RowLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        Label {
                            text: modelData.name
                            color: general.win.muted
                            Layout.fillWidth: true
                        }
                        Label {
                            text: modelData.value
                            textFormat: Text.PlainText
                            color: general.win.text
                            font.family: general.win.mono
                        }
                    }
                }
                Label {
                    Layout.topMargin: 4
                    text: qsTr("Change location and calculation method with the salah CLI.")
                    color: general.win.muted
                    font.pixelSize: 12
                }
            }
        }
    }
}
