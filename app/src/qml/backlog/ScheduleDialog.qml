import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// Schedule a backlog task: a day (Today, Tomorrow or a typed date) and a block.
Popup {
    id: dialog
    objectName: "scheduleDialog"
    required property var win
    property var task: null
    property string choice: "today"
    property string block: "fajr"
    property string waitingId: ""
    property string serverError: ""
    readonly property string todayText: backend.todayDay
    readonly property string tomorrowText: addDays(todayText, 1)
    readonly property bool customValid: /^\d{4}-\d{2}-\d{2}$/.test(dateField.text.trim())
        && !isNaN(parse(dateField.text.trim()).getTime())
        && toIso(parse(dateField.text.trim())) === dateField.text.trim()
        && dateField.text.trim() >= todayText
    readonly property string day: choice === "today" ? todayText
        : choice === "tomorrow" ? tomorrowText : dateField.text.trim()
    readonly property bool canSave: task !== null && waitingId === ""
        && !backend.offlineMode && (choice !== "date" || customValid)

    function parse(text) { return new Date(text + "T12:00:00") }
    function toIso(d) {
        function two(n) { return (n < 10 ? "0" : "") + n }
        return d.getFullYear() + "-" + two(d.getMonth() + 1) + "-" + two(d.getDate())
    }
    function addDays(text, n) {
        var d = parse(text)
        d.setDate(d.getDate() + n)
        return toIso(d)
    }
    function show(item) {
        task = item
        choice = "today"
        block = "fajr"
        waitingId = ""
        serverError = ""
        dateField.text = tomorrowText
        open()
    }
    function submit() {
        if (!canSave) return
        serverError = ""
        waitingId = task.id
        backend.scheduleItem(task.id, day, block)
    }

    modal: true
    focus: true
    anchors.centerIn: Overlay.overlay
    width: Math.min(560, win.width - 80)
    height: body.implicitHeight + 48
    padding: 0
    closePolicy: Popup.CloseOnEscape
    Overlay.modal: Rectangle { color: "#b3000000" }
    background: Rectangle {
        color: "#1c1b18"
        radius: Math.max(8, theme.radius)
        border.width: 1
        border.color: dialog.win.border
    }

    Connections {
        target: backend
        function onScheduleFinished(id, ok, message) {
            if (id !== dialog.waitingId) return
            dialog.waitingId = ""
            if (ok) dialog.close()
            else dialog.serverError = message
        }
    }

    ColumnLayout {
        id: body
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 24
        spacing: 14

        Label {
            text: qsTr("Schedule task")
            color: dialog.win.text
            font.pixelSize: 20
            font.weight: Font.DemiBold
        }
        Label {
            Layout.fillWidth: true
            text: dialog.task ? dialog.task.title : ""
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: dialog.win.muted
        }
        Label {
            text: qsTr("Day")
            color: dialog.win.muted
            font.pixelSize: 12
        }
        Flow {
            Layout.fillWidth: true
            spacing: 8
            ChoiceChip {
                objectName: "scheduleToday"
                win: dialog.win
                text: qsTr("Today")
                selected: dialog.choice === "today"
                onClicked: dialog.choice = "today"
            }
            ChoiceChip {
                objectName: "scheduleTomorrow"
                win: dialog.win
                text: qsTr("Tomorrow")
                selected: dialog.choice === "tomorrow"
                onClicked: dialog.choice = "tomorrow"
            }
            ChoiceChip {
                objectName: "scheduleDate"
                win: dialog.win
                text: qsTr("Date")
                selected: dialog.choice === "date"
                onClicked: { dialog.choice = "date"; dateField.forceActiveFocus() }
            }
        }
        Rectangle {
            visible: dialog.choice === "date"
            Layout.fillWidth: true
            height: 42
            radius: theme.radius
            color: "transparent"
            border.color: dialog.customValid ? dialog.win.border : dialog.win.muted
            TextInput {
                id: dateField
                objectName: "scheduleDateField"
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 12
                verticalAlignment: TextInput.AlignVCenter
                color: dialog.win.text
                selectionColor: theme.accent
                selectedTextColor: theme.accentForeground
                font.family: dialog.win.mono
                font.pixelSize: 15
                inputMask: "0000-00-00"
                onAccepted: dialog.submit()
            }
        }
        Label {
            visible: dialog.choice === "date" && !dialog.customValid
            text: qsTr("Use a date from today on, as YYYY-MM-DD.")
            color: dialog.win.muted
            font.pixelSize: 12
        }
        Label {
            text: qsTr("Block")
            color: dialog.win.muted
            font.pixelSize: 12
        }
        Flow {
            Layout.fillWidth: true
            spacing: 8
            Repeater {
                model: dialog.win.blockIds
                ChoiceChip {
                    required property string modelData
                    required property int index
                    objectName: "scheduleBlock_" + modelData
                    win: dialog.win
                    text: dialog.win.blockNames[index]
                    selected: dialog.block === modelData
                    onClicked: dialog.block = modelData
                }
            }
        }
        Label {
            visible: dialog.serverError !== ""
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: dialog.serverError
            textFormat: Text.PlainText
            color: dialog.win.muted
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 6
            spacing: 10
            Item { Layout.fillWidth: true }
            AppButton {
                win: dialog.win
                text: qsTr("Cancel")
                onClicked: dialog.close()
            }
            AppButton {
                objectName: "scheduleConfirm"
                win: dialog.win
                text: qsTr("Schedule")
                primary: true
                enabled: dialog.canSave
                onClicked: dialog.submit()
            }
        }
    }
}
