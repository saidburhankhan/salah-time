import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui

Column {
  id: editor

  required property var host
  readonly property string fontFamily: host.bar ? host.bar.fontFamily : Style.font.family
  spacing: Style.space(3)

  function focusInput() {
    if (visible) input.focusInput()
  }

  Component.onCompleted: Qt.callLater(focusInput)

  Connections {
    target: editor.host
    function onNoteFocusRequested() { editor.focusInput() }
  }

  NoteInput {
    id: input
    width: parent.width
    text: editor.host.noteDraft
    placeholderText: editor.host.tr.t("session.notePlaceholder")
    foreground: editor.host.foreground
    fontFamily: editor.fontFamily
    readOnly: editor.host.noteOp !== ""
    onTextChanged: if (visible && editor.host.noteSource !== "") editor.host.noteDraft = text
    onTextFocusChanged: {
      if (textFocus) editor.host.focusedNoteField = input
      else if (editor.host.focusedNoteField === input) editor.host.focusedNoteField = null
    }
    onAccepted: editor.host.saveNote()
    onCanceled: editor.host.closeNote()
  }

  Row {
    anchors.right: parent.right
    spacing: Style.space(6)

    Button {
      height: Style.space(24)
      text: editor.host.tr.t("session.cancel")
      enabled: editor.host.noteOp === ""
      foreground: editor.host.mutedForeground
      fontFamily: editor.fontFamily
      fontSize: Style.font.bodySmall
      verticalPadding: 0
      onClicked: editor.host.closeNote()
    }

    Button {
      height: Style.space(24)
      text: editor.host.tr.t(editor.host.noteOp === "" ? "session.save" : "session.saving")
      enabled: editor.host.noteOp === ""
      bordered: true
      foreground: editor.host.foreground
      fontFamily: editor.fontFamily
      fontSize: Style.font.bodySmall
      verticalPadding: 0
      onClicked: editor.host.saveNote()
    }
  }

  Text {
    visible: text !== ""
    width: parent.width
    textFormat: Text.PlainText
    text: editor.host.noteError
    color: Commons.Color.urgent
    font.family: editor.fontFamily
    font.pixelSize: Style.font.caption
    wrapMode: Text.Wrap
  }
}
