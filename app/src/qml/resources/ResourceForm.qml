import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "ResourceTime.js" as Time

// Add or edit one resource. Everything is optional except what a kind needs
// to mean anything: a link needs a valid URL, a reminder a date and time.
Rectangle {
    id: form
    required property var win
    // Empty for a new resource, else the row being edited (its kind is fixed).
    property var resource: null
    property string kind: "note"
    property string serverError: ""
    property bool waiting: false
    readonly property bool editing: resource !== null
    readonly property string instant: Time.toInstant(dateField.text, timeField.text)
    readonly property bool valid: kind === "link" ? Time.validUrl(urlField.text)
        : kind === "reminder" ? instant !== "" : true
    signal submitted(var fields)
    signal cancelled()

    function load(res) {
        resource = res
        kind = res ? String(res.kind) : "note"
        serverError = ""
        waiting = false
        bodyField.text = res && res.body ? String(res.body) : ""
        urlField.text = res && res.url ? String(res.url) : ""
        titleField.text = res && res.title ? String(res.title) : ""
        var when = res && res.remind_at ? new Date(res.remind_at) : Time.daysFromNow(1, 9)
        dateField.text = Time.dateText(when)
        timeField.text = Time.timeText(when)
    }
    function setWhen(d) { dateField.text = Time.dateText(d); timeField.text = Time.timeText(d) }
    function submit() {
        if (!valid || waiting) return
        var fields = {}
        if (kind === "note") fields = { body: bodyField.text }
        else if (kind === "link") fields = { url: urlField.text.trim(), title: titleField.text }
        else fields = { title: titleField.text, remind_at: instant }
        if (!editing) fields.kind = kind
        serverError = ""
        waiting = true
        submitted(fields)
    }

    implicitHeight: column.implicitHeight + 28
    radius: theme.radius
    color: "#1c1b18"
    border.color: win.selectedBorder

    ColumnLayout {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 14
        spacing: 10

        Flow {
            visible: !form.editing
            Layout.fillWidth: true
            spacing: 8
            Repeater {
                model: [{ id: "note", name: qsTr("Note") }, { id: "link", name: qsTr("Link") },
                        { id: "reminder", name: qsTr("Reminder") }]
                ChoiceChip {
                    required property var modelData
                    objectName: "resourceKind_" + modelData.id
                    win: form.win
                    implicitHeight: 34
                    text: modelData.name
                    selected: form.kind === modelData.id
                    onClicked: { form.kind = modelData.id; form.serverError = "" }
                }
            }
        }
        Label {
            visible: form.editing
            text: form.kind === "note" ? qsTr("Edit note") : form.kind === "link" ? qsTr("Edit link") : qsTr("Edit reminder")
            color: form.win.muted
            font.pixelSize: 12
        }

        TextArea {
            id: bodyField
            objectName: "resourceBody"
            visible: form.kind === "note"
            Layout.fillWidth: true
            Layout.preferredHeight: 110
            placeholderText: qsTr("Note")
            wrapMode: TextEdit.Wrap
            textFormat: TextEdit.PlainText
        }
        TextField {
            id: urlField
            objectName: "resourceUrl"
            visible: form.kind === "link"
            Layout.fillWidth: true
            placeholderText: qsTr("https://…")
            font.family: form.win.mono
            onAccepted: form.submit()
        }
        Label {
            visible: form.kind === "link" && urlField.text.trim() !== "" && !Time.validUrl(urlField.text)
            text: qsTr("Use a full http or https address.")
            color: form.win.muted
            font.pixelSize: 12
        }
        TextField {
            id: titleField
            objectName: "resourceTitle"
            visible: form.kind !== "note"
            Layout.fillWidth: true
            placeholderText: form.kind === "link" ? qsTr("Title (optional)") : qsTr("Reminder text (optional)")
            onAccepted: form.submit()
        }
        ColumnLayout {
            visible: form.kind === "reminder"
            Layout.fillWidth: true
            spacing: 8
            RowLayout {
                spacing: 8
                TextField {
                    id: dateField
                    objectName: "resourceDate"
                    Layout.preferredWidth: 130
                    font.family: form.win.mono
                    inputMask: "0000-00-00"
                    inputMethodHints: Qt.ImhDigitsOnly
                }
                TextField {
                    id: timeField
                    objectName: "resourceTime"
                    Layout.preferredWidth: 84
                    font.family: form.win.mono
                    inputMask: "00:00"
                    inputMethodHints: Qt.ImhDigitsOnly
                }
                Label {
                    visible: form.instant === ""
                    Layout.fillWidth: true
                    text: qsTr("Pick a real date and time.")
                    color: form.win.muted
                    font.pixelSize: 12
                    wrapMode: Text.Wrap
                }
            }
            Flow {
                Layout.fillWidth: true
                spacing: 6
                ChoiceChip {
                    win: form.win
                    implicitHeight: 32
                    text: qsTr("In 1 h")
                    onClicked: form.setWhen(new Date(Date.now() + 3600000))
                }
                ChoiceChip {
                    win: form.win
                    implicitHeight: 32
                    text: qsTr("Today 18:00")
                    onClicked: form.setWhen(Time.daysFromNow(0, 18))
                }
                ChoiceChip {
                    objectName: "resourceTomorrow"
                    win: form.win
                    implicitHeight: 32
                    text: qsTr("Tomorrow 09:00")
                    onClicked: form.setWhen(Time.daysFromNow(1, 9))
                }
                ChoiceChip {
                    win: form.win
                    implicitHeight: 32
                    text: qsTr("Next week")
                    onClicked: form.setWhen(Time.daysFromNow(7, 9))
                }
            }
        }
        Label {
            visible: form.serverError !== ""
            Layout.fillWidth: true
            text: form.serverError
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: "#c4876b"
            font.pixelSize: 12
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Item { Layout.fillWidth: true }
            AppButton {
                objectName: "resourceCancel"
                win: form.win
                text: qsTr("Cancel")
                implicitHeight: 34
                onClicked: form.cancelled()
            }
            AppButton {
                objectName: "resourceSave"
                win: form.win
                text: qsTr("Save")
                primary: true
                implicitHeight: 34
                enabled: form.valid && !form.waiting
                onClicked: form.submit()
            }
        }
    }
}
