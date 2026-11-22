import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

ScrollView {
    id: entryFields
    property var win
    property var form
    property alias titleText: titleField.text
    property alias goalText: plannedField.text
    property alias projectIndex: projectPicker.currentIndex
    Layout.fillWidth: true
    Layout.fillHeight: true
    Layout.minimumWidth: 450
    clip: true
    function initialize(title, target) {
        titleField.text = title
        plannedField.text = target
    }
    function clearTitle() {
        titleField.text = ""
    }
    Column {
        width: entryFields.availableWidth
        spacing: 18
        Rectangle {
            width: parent.width
            height: 108
            radius: theme.radius
            color: form.win.panel
            border.color: form.win.border
            Item {
                anchors.fill: parent
                anchors.margins: 24
                Text {
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    text: qsTr("Activity title…")
                    color: form.win.muted
                    font.pixelSize: 24
                    visible: titleField.text.length === 0
                }
                TextInput {
                    id: titleField
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    color: form.win.text
                    selectionColor: theme.accent
                    selectedTextColor: theme.accentForeground
                    font.pixelSize: 28
                    clip: true
                    onAccepted: form.submitTitle()
                }
                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: 1
                    color: form.win.border
                }
            }
        }
        Rectangle {
            width: parent.width
            height: 150
            radius: theme.radius
            color: form.win.panel
            border.color: form.win.border
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 22
                spacing: 16
                Label {
                    text: qsTr("When")
                    color: form.win.text
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                }
                RowLayout {
                    spacing: 8
                    Repeater {
                        model: form.win.blockIds
                        ChoiceChip {
                            required property string modelData
                            required property int index
                            win: form.win
                            text: form.win.blockNames[index]
                            selected: form.block === modelData
                            onClicked: form.block = modelData
                        }
                    }
                }
            }
        }
        Rectangle {
            width: parent.width
            height: 160
            radius: theme.radius
            color: form.win.panel
            border.color: form.win.border
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 22
                spacing: 14
                Label {
                    text: qsTr("Project and plan")
                    color: form.win.text
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                }
                RowLayout {
                    spacing: 8
                    ComboBox { id: projectPicker; model: [qsTr("Routine / no project")].concat((backend.projects || []).map(function(p){ return String(p.name || "") })) }
                    Label {
                        text: qsTr("Planned minutes")
                        color: form.win.muted
                        Layout.leftMargin: 10
                    }
                    TextField {
                        id: plannedField
                        Layout.preferredWidth: 110
                        placeholderText: qsTr("Optional")
                        font.family: form.win.mono
                        inputMethodHints: Qt.ImhDigitsOnly
                        validator: IntValidator { bottom: 1; top: 99999 }
                    }
                }
            }
        }
        Rectangle {
            width: parent.width
            height: form.repeat === "weekly" ? 220 : 150
            radius: theme.radius
            color: form.win.panel
            border.color: form.win.border
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 22
                spacing: 12
                Label {
                    text: qsTr("Repeat")
                    color: form.win.text
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                }
                RowLayout {
                    spacing: 8
                    Repeater {
                        model: [{ id: "none", name: qsTr("Once") },
                            { id: "daily", name: qsTr("Daily") },
                            { id: "weekdays", name: qsTr("Weekdays") },
                            { id: "weekly", name: qsTr("Weekly") }]
                        ChoiceChip {
                            required property var modelData
                            win: form.win
                            text: modelData.name
                            selected: modelData.id !== "weekdays"
                                && form.repeat === modelData.id
                            onClicked: form.selectRepeat(modelData.id)
                        }
                    }
                }
                RowLayout {
                    visible: form.repeat === "weekly"
                    spacing: 6
                    Repeater {
                        model: ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
                        ChoiceChip {
                            required property string modelData
                            win: form.win
                            text: form.win.capitalized(modelData)
                            selected: form.days.indexOf(modelData) >= 0
                            onClicked: form.toggleDay(modelData)
                        }
                    }
                }
                Label {
                    visible: form.repeat === "weekly" && form.days.length === 0
                    text: qsTr("Pick at least one day")
                    color: form.win.muted
                    font.pixelSize: 12
                }
            }
        }
    }
}
