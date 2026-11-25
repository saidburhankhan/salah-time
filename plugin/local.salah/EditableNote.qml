import QtQuick
import qs.Commons
import qs.Ui

// A display row and its editor share the same place in the parent layout.
Column {
  id: row

  required property var host
  required property string source
  required property var target
  property string timestamp: ""
  readonly property bool editing: host.noteSource === source && source !== ""
  readonly property string fontFamily: host.bar ? host.bar.fontFamily : Style.font.family
  visible: editing || !!target && target.original !== ""

  function openEditor() { host.beginNote(source, target) }

  Row {
    id: reading
    visible: !row.editing
    width: parent.width
    spacing: Style.space(4)

    Text {
      id: stamp
      visible: text !== ""
      textFormat: Text.PlainText
      text: row.timestamp
      color: row.host.mutedForeground
      font.family: row.fontFamily
      font.pixelSize: Style.font.caption
    }

    Text {
      id: label
      width: Math.max(0, parent.width - pencil.width - reading.spacing
        - (stamp.visible ? stamp.width + reading.spacing : 0))
      textFormat: Text.PlainText
      text: row.target ? row.target.original : ""
      color: row.host.foreground
      font.family: row.fontFamily
      font.pixelSize: Style.font.bodySmall
      wrapMode: Text.Wrap

      MouseArea {
        id: noteHit
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: row.openEditor()
      }
    }

    Button {
      id: pencil
      width: Style.space(21)
      height: Style.space(20)
      text: "✎"
      tooltipText: row.host.tr.t("session.editNote")
      foreground: row.host.mutedForeground
      fontFamily: row.fontFamily
      fontSize: Style.font.bodySmall
      horizontalPadding: 0
      verticalPadding: 0
      focusable: true
      opacity: hot || activeFocus || noteHit.containsMouse ? 1 : 0.55
      onClicked: row.openEditor()
    }
  }

  Loader {
    active: row.editing
    visible: active
    width: parent.width
    sourceComponent: SessionNoteField {
      host: row.host
    }
  }
}
