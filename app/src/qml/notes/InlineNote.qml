import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

ColumnLayout {
    id: note
    required property var win
    required property var session
    property var entry: null
    property string text: entry ? String(entry.text || "") : String(session.note || "")
    property string prefix: entry ? win.clockTime(entry.at) : ""
    property int pixelSize: 13
    property string buttonName: "inlineNoteText"
    property bool removable: false
    property bool previewWhileRemoving: editor.removing
    readonly property bool editing: editor.draft !== null
    readonly property bool editable: !!session.id && session.version != null
    signal saved(var session)
    signal canceled()
    Layout.fillWidth: true
    visible: editing || !!text
    spacing: 4

    function start(action) {
        if (editable && !editing)
            editor.begin(action, session.id, {id: entry?.id || "", text: text}, session.version)
    }

    RowLayout {
        Layout.fillWidth: true
        visible: !note.editing || note.previewWhileRemoving
        spacing: 6
        Label {
            visible: !!note.prefix
            text: note.prefix
            textFormat: Text.PlainText
            font.family: note.win.mono
            font.pixelSize: 11
            color: note.win.muted
        }
        Button {
            id: textButton
            objectName: note.buttonName
            Layout.fillWidth: true
            padding: 0
            hoverEnabled: true
            enabled: note.editable && !note.editing
            text: note.text
            Accessible.description: qsTr("Edit note")
            contentItem: Label {
                text: textButton.text
                textFormat: Text.PlainText
                font.pixelSize: note.pixelSize
                color: note.win.text
                wrapMode: Text.Wrap
            }
            background: Rectangle {
                radius: 4
                color: textButton.hovered ? note.win.card : "transparent"
                border.color: textButton.visualFocus ? note.win.selectedBorder : "transparent"
            }
            function submitForm() { note.start(note.entry ? "edit" : "main") }
            onClicked: submitForm()
        }
        Button {
            id: pencil
            objectName: "inlineNotePencil"
            text: "✎"
            Accessible.name: qsTr("Edit note")
            hoverEnabled: true
            enabled: note.editable && !note.editing
            padding: 2
            opacity: note.editable && (textButton.hovered || textButton.activeFocus || hovered || activeFocus) ? 1 : 0
            contentItem: Label {
                text: pencil.text
                color: note.win.muted
                horizontalAlignment: Text.AlignHCenter
            }
            background: Rectangle {
                color: note.win.panel
                border.color: pencil.visualFocus ? note.win.selectedBorder : note.win.border
                radius: 4
            }
            function submitForm() { note.start(note.entry ? "edit" : "main") }
            onClicked: submitForm()
        }
        AppButton {
            visible: note.removable
            objectName: "removeTimedNote_" + (note.entry?.id || "")
            win: note.win
            enabled: note.editable && !note.editing
            text: qsTr("Remove")
            onClicked: note.start("remove")
        }
    }
    NoteEditor {
        id: editor
        visible: note.editing
        Layout.fillWidth: true
        win: note.win
        onCompleted: session => note.saved(session)
        onCanceled: {
            textButton.forceActiveFocus()
            note.canceled()
        }
    }
}
