import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

Popup {
    id: dialog
    objectName: "routineEndDialog"
    required property var win
    property var task: ({})
    property var stageDate: null
    property bool pickDate: false
    property bool waiting: false
    property string errorText: ""
    readonly property string endDay: pickDate ? dateField.day : backend.todayDay
    readonly property bool valid: pickDate ? dateField.valid : endDay >= dateField.minimumDay && !!endDay
    modal: true
    focus: true
    anchors.centerIn: Overlay.overlay
    width: Math.min(500, win.width - 48)
    padding: 22
    closePolicy: Popup.NoAutoClose
    Overlay.modal: Rectangle { color: "#b3000000" }
    background: Card { win: dialog.win }

    function show(item, applyToDraft) {
        task = item
        stageDate = applyToDraft || null
        waiting = false
        errorText = ""
        var today = backend.todayDay
        dateField.text = String(item.end_day || (today >= (item.day || "") ? today : item.day))
        pickDate = dateField.text !== today
        open()
    }
    function submit(day) {
        if (waiting || !task.id || (day && !valid)) return
        if (stageDate) {
            stageDate(day)
            close()
        } else {
            errorText = ""
            waiting = true
            backend.setRoutineEndDay(task.id, day, task.version ?? -1)
        }
    }

    Connections {
        target: backend
        function onRoutineEditFinished(id, ok, error) {
            if (!dialog.waiting || id !== dialog.task.id) return
            dialog.waiting = false
            if (ok) dialog.close()
            else dialog.errorText = error
        }
    }

    contentItem: ColumnLayout {
        focus: true
        function submitForm() { dialog.submit(dialog.endDay) }
        function cancelForm() { if (!dialog.waiting) dialog.close() }
        spacing: 12
        Label {
            text: qsTr("End routine")
            color: dialog.win.text
            font.pixelSize: 20
            font.weight: Font.DemiBold
        }
        Label {
            Layout.fillWidth: true
            text: String(dialog.task.title || "")
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: dialog.win.text
        }
        Label {
            Layout.fillWidth: true
            text: qsTr("Choose the routine’s last tracking day. Past days and sessions are kept.")
            color: dialog.win.muted
            wrapMode: Text.WordWrap
        }
        RowLayout {
            ChoiceChip {
                objectName: "routineEndToday"
                win: dialog.win
                text: qsTr("Today")
                selected: !dialog.pickDate
                onClicked: dialog.pickDate = false
            }
            ChoiceChip {
                objectName: "routineEndPickDate"
                win: dialog.win
                text: qsTr("Pick date")
                selected: dialog.pickDate
                onClicked: { dialog.pickDate = true; dateField.forceActiveFocus() }
            }
        }
        TrackingDateField {
            id: dateField
            objectName: "routineEndDate"
            visible: dialog.pickDate
            Layout.fillWidth: true
            minimumDay: String(dialog.task.day || "")
            Accessible.name: qsTr("Last tracking day, YYYY-MM-DD")
            onAccepted: dialog.submit(dialog.endDay)
        }
        Label {
            visible: !dialog.valid
            Layout.fillWidth: true
            text: qsTr("Choose a valid date on or after the routine’s first day.")
            color: dialog.win.muted
            wrapMode: Text.WordWrap
        }
        Label {
            visible: !!dialog.errorText
            Layout.fillWidth: true
            text: dialog.errorText
            textFormat: Text.PlainText
            color: dialog.win.muted
            wrapMode: Text.WordWrap
        }
        RowLayout {
            Layout.fillWidth: true
            AppButton {
                visible: !!dialog.task.end_day && !dialog.stageDate
                win: dialog.win
                text: qsTr("Resume")
                enabled: !dialog.waiting
                onClicked: dialog.submit("")
            }
            Item { Layout.fillWidth: true }
            AppButton {
                win: dialog.win
                text: qsTr("Cancel")
                enabled: !dialog.waiting
                onClicked: dialog.close()
            }
            AppButton {
                objectName: "routineEndConfirm"
                win: dialog.win
                primary: true
                text: dialog.stageDate ? qsTr("Use date") : qsTr("End routine")
                enabled: dialog.valid && !dialog.waiting
                onClicked: dialog.submit(dialog.endDay)
            }
        }
    }
}
