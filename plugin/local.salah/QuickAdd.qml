import QtQuick
import qs.Commons
import qs.Ui

// Quick-add field and its transient hints. Kept separate so the panel content
// only wires the controllers together.
Column {
  id: root

  property var strings
  property var host
  property string fontFamily
  property color foreground
  property color mutedForeground
  property string occurrenceHint: ""
  property string actionError: ""
  property alias quickField: quickField

  width: parent ? parent.width : 0
  spacing: Style.space(3)

  TextField {
    id: quickField
    width: parent.width
    placeholderText: root.strings.t("panel.quickAddPlaceholder")
    foreground: root.foreground
    font.family: root.fontFamily
    maximumLength: 200

    onAccepted: {
      root.host.addItem(text)
      text = ""
    }

    Keys.onEscapePressed: function(event) {
      root.host.clearQuickAddPreset()
      text = ""
      focus = false
      event.accepted = true
    }
  }

  Text {
    visible: root.host.quickAddBlock !== ""
    width: parent.width
    textFormat: Text.PlainText
    text: root.strings.t("panel.addingTo").arg(root.host.prayerName(root.host.quickAddBlock, root.host.quickAddBlock))
    color: root.mutedForeground
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    elide: Text.ElideRight
  }

  Text {
    visible: root.occurrenceHint !== ""
    width: parent.width
    textFormat: Text.PlainText
    text: root.occurrenceHint
    color: root.mutedForeground
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    elide: Text.ElideRight
  }

  Text {
    visible: root.actionError !== ""
    width: parent.width
    textFormat: Text.PlainText
    text: root.actionError
    color: root.mutedForeground
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    wrapMode: Text.WrapAnywhere
    maximumLineCount: 2
    elide: Text.ElideRight
  }
}
