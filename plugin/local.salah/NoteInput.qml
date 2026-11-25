import QtQuick
import QtQuick.Controls as Controls
import qs.Commons
import qs.Commons as Commons
import qs.Ui

Controls.ScrollView {
  id: field

  property alias text: input.text
  property alias readOnly: input.readOnly
  property alias placeholderText: input.placeholderText
  readonly property alias textFocus: input.activeFocus
  property color foreground: Commons.Color.foreground
  property string fontFamily: Style.font.family
  signal accepted()
  signal canceled()

  function focusInput() { input.forceActiveFocus() }

  height: Math.max(Style.space(36), Math.min(input.contentHeight + input.topPadding + input.bottomPadding, Style.space(92)))
  contentWidth: availableWidth
  clip: true
  background: BorderSurface {
    color: Style.controlFill(input.activeFocus, input.hovered, field.foreground, Commons.Color.accent)
    borderSpec: Border.controlSpec(input.activeFocus ? "focus" : input.hovered ? "hover-cursor" : "normal",
      field.foreground, Commons.Color.accent)
    radius: Style.cornerRadius
  }

  Controls.TextArea {
    id: input
    textFormat: TextEdit.PlainText
    wrapMode: TextEdit.Wrap
    selectByMouse: true
    color: field.foreground
    placeholderTextColor: Qt.darker(field.foreground, 1.6)
    selectionColor: Style.selectionFillFor(field.foreground, Commons.Color.accent)
    selectedTextColor: field.foreground
    font.family: field.fontFamily
    font.pixelSize: Style.font.bodySmall
    padding: Style.space(6)
    background: null

    Keys.priority: Keys.BeforeItem
    Keys.onPressed: function(event) {
      if (event.key === Qt.Key_Escape) field.canceled()
      else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
        if (event.modifiers & Qt.ShiftModifier) {
          if (!input.readOnly) {
            var start = input.selectionStart
            input.remove(start, input.selectionEnd)
            input.insert(start, "\n")
            input.cursorPosition = start + 1
          }
        } else {
          field.accepted()
        }
      } else return
      event.accepted = true
    }
  }
}
