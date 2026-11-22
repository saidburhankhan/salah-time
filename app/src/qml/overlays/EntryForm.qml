import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

Popup {
    id: entryPopup
    objectName: "entryPopup"
    property var win
    property var item: ({})
    property string block: item.block || "fajr"
    property string kind: item.measure === "count" ? "counter" : "task"
    property string repeat: item.repeat || "none"
    property var days: item.weekdays || []
    property bool confirming: false
    readonly property bool weeklyValid: repeat !== "weekly" || days.length > 0
    visible: win ? win.entryVisible : false
    modal: true
    focus: true
    anchors.centerIn: Overlay.overlay
    width: win ? Math.min(1100, win.width - 48) : 0
    height: win ? Math.min(820, win.height - 48) : 0
    padding: 0
    closePolicy: Popup.NoAutoClose
    Overlay.modal: Rectangle { color: "#d9000000" }
    background: null
    onOpened: {
        block = item.block || "fajr"
        kind = item.measure === "count" ? "counter" : "task"
        repeat = item.repeat || "none"
        days = item.weekdays || []
        confirming = false
        entryFields.initialize(item.title || "", item.target ? String(item.target) : "")
    }
    function draft() {
        var goal = Number(entryFields.goalText)
        return {
            id: item.id || "",
            title: entryFields.titleText,
            block: block,
            measure: kind === "counter" ? "count" : goal > 0 ? "minutes" : "check",
            target: goal > 0 ? goal : 0,
            repeat: repeat,
            weekdays: repeat === "weekly" ? days : []
        }
    }
    function submitTitle() {
        if (!entryFields.titleText.trim() || !weeklyValid) return
        var d = draft()
        win.maybeToast(d)
        backend.saveItem(d)
        win.entryVisible = false
    }
    function toggleDay(d) {
        var a = days.slice()
        var i = a.indexOf(d)
        if (i >= 0) a.splice(i, 1)
        else a.push(d)
        days = a
    }
    function selectRepeat(id) {
        if (id === "weekdays") {
            repeat = "weekly"
            days = ["mon", "tue", "wed", "thu", "fri"]
        } else {
            repeat = id
            if (id === "weekly" && days.length === 0) {
                var selected = Date.fromLocaleString(Qt.locale(), backend.selectedDay, "yyyy-MM-dd")
                days = [entryPopup.win.weekdayId(selected)]
            }
        }
    }
    Rectangle {
        anchors.fill: parent
        color: "#151411"
        radius: Math.max(8, theme.radius)
        border.width: 1
        border.color: entryPopup.win.border
    }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 1
        spacing: 0
        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 72
            Layout.leftMargin: 22
            Layout.rightMargin: 22
            Label {
                text: entryPopup.item.id ? qsTr("Edit entry") : qsTr("New entry")
                color: entryPopup.win.text
                font.pixelSize: 20
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }
            AppButton {
                win: entryPopup.win
                visible: !!entryPopup.item.id
                text: entryPopup.confirming ? qsTr("Confirm delete") : qsTr("Delete")
                onClicked: {
                    if (entryPopup.confirming) {
                        backend.removeItem(entryPopup.item.id)
                        entryPopup.win.entryVisible = false
                    } else {
                        entryPopup.confirming = true
                    }
                }
            }
            AppButton {
                win: entryPopup.win
                text: qsTr("Cancel")
                onClicked: entryPopup.win.entryVisible = false
            }
            AppButton {
                win: entryPopup.win
                text: qsTr("Save & add another")
                enabled: entryFields.titleText.trim() !== "" && entryPopup.weeklyValid
                onClicked: {
                    var d = entryPopup.draft()
                    entryPopup.win.maybeToast(d)
                    backend.saveItem(d)
                    entryPopup.win.editItem = ({})
                    entryFields.clearTitle()
                }
            }
            AppButton {
                win: entryPopup.win
                text: qsTr("Save  ↵")
                primary: true
                enabled: entryFields.titleText.trim() !== "" && entryPopup.weeklyValid
                onClicked: {
                    var d = entryPopup.draft()
                    entryPopup.win.maybeToast(d)
                    backend.saveItem(d)
                    entryPopup.win.entryVisible = false
                }
            }
        }
        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: entryPopup.win.border
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: 22
            spacing: 26
            EntryFields {
                id: entryFields
                win: entryPopup.win
                form: entryPopup
            }
            ColumnLayout {
                Layout.preferredWidth: 340
                Layout.minimumWidth: 280
                Layout.maximumWidth: 360
                Layout.fillHeight: true
                Rectangle {
                    Layout.fillWidth: true
                    height: 245
                    radius: theme.radius
                    color: entryPopup.win.panel
                    border.color: entryPopup.win.border
                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 22
                        spacing: 14
                        Label {
                            text: qsTr("Preview")
                            color: entryPopup.win.text
                            font.pixelSize: 18
                            font.weight: Font.DemiBold
                        }
                        Rectangle {
                            Layout.fillWidth: true
                            height: 1
                            color: entryPopup.win.border
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                text: qsTr("Next occurrence")
                                color: entryPopup.win.muted
                                Layout.fillWidth: true
                            }
                            Label {
                                text: backend.gregorianHeader(backend.selectedDay)
                                color: entryPopup.win.text
                                font.family: entryPopup.win.mono
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                text: qsTr("Occurrences this month")
                                color: entryPopup.win.muted
                                Layout.fillWidth: true
                            }
                            Label {
                                text: entryPopup.repeat === "daily"
                                    ? 30
                                    : entryPopup.repeat === "weekly"
                                        ? Math.max(1, entryPopup.days.length * 4)
                                        : 1
                                color: entryPopup.win.text
                                font.family: entryPopup.win.mono
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                text: qsTr("Schedule")
                                color: entryPopup.win.muted
                                Layout.fillWidth: true
                            }
                            Label {
                                text: entryPopup.win.weekdayText(entryPopup.days, entryPopup.repeat)
                                color: entryPopup.win.text
                                font.family: entryPopup.win.mono
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                text: entryPopup.kind === "task" ? qsTr("Time goal") : qsTr("Goal")
                                color: entryPopup.win.muted
                                Layout.fillWidth: true
                            }
                            Label {
                                text: entryFields.goalText
                                    ? (entryFields.goalText + (entryPopup.kind === "task" ? qsTr(" min") : ""))
                                    : qsTr("None")
                                color: entryPopup.win.text
                                font.family: entryPopup.win.mono
                            }
                        }
                    }
                }
                Item { Layout.fillHeight: true }
            }
        }
    }
}
