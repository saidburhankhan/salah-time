import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Basic as Basic
import QtQuick.Layouts
import "../controls"
import "../controls/FormKeys.js" as FormKeys

ColumnLayout {
    id: editor
    required property var win
    property var draft: null
    property var pending: null
    property string operation: ""
    property string errorText: ""
    property string original: ""
    readonly property bool dirty: waiting || frozen || (!removing
        && (body.text !== original || (adding && timeField.text !== "")))
    function submitForm() { submit() }
    function cancelForm() { cancel() }
    function focusDraft() {
        if (adding) timeField.forceActiveFocus()
        else if (!removing) body.forceActiveFocus()
        else confirm.forceActiveFocus()
    }
    Component.onDestruction: if (win) win.releaseNoteEditor(editor)
    readonly property bool waiting: operation !== ""
    readonly property bool frozen: pending !== null
    readonly property bool adding: draft?.action === "add"
    readonly property bool removing: draft?.action === "remove"
    readonly property bool valid: removing || (!!body.text.trim() && (!adding || timeField.acceptableInput))
    signal completed(var session)
    signal canceled()
    spacing: 8

    function begin(action, id, note, version) {
        if (!win.claimNoteEditor(editor)) return false
        draft = { action: action, sessionId: id, noteId: note?.id || "", version: version ?? -1 }
        pending = null
        operation = ""
        errorText = ""
        original = action === "edit" || action === "main" ? String(note?.text || "") : ""
        body.text = original
        timeField.text = ""
        Qt.callLater(focusDraft)
        return true
    }

    function reset(keepOwner) {
        draft = null
        pending = null
        operation = ""
        errorText = ""
        if (!keepOwner) win.releaseNoteEditor(editor)
    }

    function cancel() {
        if (waiting) return
        reset(true)
        canceled()
        win.releaseNoteEditor(editor)
    }

    function submit() {
        if (!draft || waiting || !valid) return
        if (!pending) {
            pending = {
                op: backend.newOperationId(),
                fields: Object.assign({}, draft, { text: body.text, at: adding ? timeField.text : "" })
            }
        }
        operation = pending.op
        errorText = ""
        backend.writeSessionNote(pending.fields, operation)
    }

    Connections {
        target: backend
        function onSessionNoteWriteFinished(op, ok, uncertain, session, error) {
            if (!editor.waiting || op !== editor.operation) return
            editor.operation = ""
            if (ok) {
                editor.reset(true)
                editor.completed(session)
                editor.win.releaseNoteEditor(editor)
            } else {
                if (!uncertain) editor.pending = null
                editor.errorText = uncertain ? qsTr("%1 Retry to confirm this change.").arg(error) : error
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        visible: editor.adding
        spacing: 12
        TextField {
            id: timeField
            objectName: "timedNoteTimeField"
            Layout.preferredWidth: 100
            implicitHeight: 38
            placeholderText: "HH:MM"
            Accessible.name: qsTr("Note time")
            font.family: editor.win.mono
            readOnly: editor.frozen
            validator: RegularExpressionValidator { regularExpression: /(?:[01][0-9]|2[0-3]):[0-5][0-9]/ }
            Keys.onPressed: event => FormKeys.route(event, editor.win)
            onTextChanged: if (!editor.frozen) editor.errorText = ""
        }
        Label {
            Layout.fillWidth: true
            text: qsTr("Local time during the session")
            color: editor.win.muted
            wrapMode: Text.WordWrap
        }
    }

    ScrollView {
        visible: !editor.removing
        Layout.fillWidth: true
        Layout.preferredHeight: 86
        clip: true
        Basic.TextArea {
            id: body
            Keys.onPressed: event => FormKeys.multiline(event, body, editor.submit, editor.cancel)
            objectName: "editableNoteText"
            Accessible.name: qsTr("Note text")
            placeholderText: qsTr("Note text")
            padding: 10
            selectionColor: editor.win.selectedFillPressed
            selectedTextColor: editor.win.selectedText
            onTextChanged: if (!editor.frozen) editor.errorText = ""
            readOnly: editor.frozen
            color: editor.win.text
            placeholderTextColor: editor.win.muted
            wrapMode: TextEdit.Wrap
            background: Rectangle {
                color: editor.win.panel
                border.color: body.activeFocus ? editor.win.selectedBorder : editor.win.border
                radius: theme.radius
            }
            textFormat: TextEdit.PlainText
        }
    }
    Label {
        visible: editor.removing
        text: qsTr("Remove this timed note?")
        color: editor.win.muted
    }
    Label {
        visible: !!editor.errorText
        Layout.fillWidth: true
        text: editor.errorText
        textFormat: Text.PlainText
        color: "#ef8068"
        wrapMode: Text.Wrap
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        Item { Layout.fillWidth: true }
        AppButton {
            win: editor.win
            text: qsTr("Cancel")
            enabled: !editor.waiting
            onClicked: editor.cancel()
        }
        AppButton {
            id: confirm
            objectName: "editableNoteSave"
            win: editor.win
            primary: !editor.removing
            text: editor.waiting ? qsTr("Saving…") : editor.frozen ? qsTr("Retry")
                : editor.removing ? qsTr("Remove note") : editor.adding ? qsTr("Add note") : qsTr("Save note")
            enabled: !editor.waiting && editor.valid
            onClicked: editor.submit()
        }
    }
}
