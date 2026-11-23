import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui

Column {
  id: editor

  required property var host
  readonly property string fontFamily: host.bar ? host.bar.fontFamily : Style.font.family
  readonly property alias textFocus: input.activeFocus
  spacing: Style.space(3)

  function focusInput() {
    if (visible) input.forceActiveFocus()
  }

  Row {
    width: parent.width
    spacing: Style.space(6)

    TextField {
      id: input
      width: parent.width - save.width - parent.spacing
      height: Style.space(28)
      maximumLength: 500
      text: editor.host.noteDraft
      placeholderText: editor.host.tr.t("session.addNote")
      foreground: editor.host.foreground
      font.family: editor.fontFamily
      font.pixelSize: Style.font.bodySmall
      verticalPadding: 0
      readOnly: editor.host.noteOp !== ""
      onTextEdited: editor.host.noteDraft = text
      onAccepted: editor.host.saveNote()
      Keys.onEscapePressed: function(event) {
        editor.host.closeNote()
        event.accepted = true
      }
    }

    Button {
      id: save
      height: input.height
      text: editor.host.tr.t("session.save")
      enabled: editor.host.noteDraft.trim() !== "" && editor.host.noteOp === ""
      bordered: true
      foreground: editor.host.foreground
      fontFamily: editor.fontFamily
      fontSize: Style.font.bodySmall
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
