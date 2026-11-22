import QtQuick
import qs.Commons
import qs.Ui

// Expanded-row controls: edit and a two-step delete. Session minutes are
// recorded only through the session flow.
Item {
  id: root

  property var strings
  property var host
  property var controller
  property var itemData
  property string rowKey
  property string fontFamily
  property color foreground
  property color mutedForeground
  property string repeat
  property bool titleEditing: false
  signal beginEdit()
  signal removeConfirmed()

  height: Style.space(38)

  function focusCorrection() {}

  onVisibleChanged: if (!visible) {
    confirmingDelete = false
  }

  property bool confirmingDelete: false

  Row {
    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    spacing: Style.space(4)

    Button {
      visible: root.repeat === "none" && !root.confirmingDelete
      width: Style.space(42)
      height: Style.space(28)
      text: root.strings.t("panel.edit")
      foreground: root.mutedForeground
      fontFamily: root.fontFamily
      horizontalPadding: Style.space(3)
      verticalPadding: 0
      onClicked: root.beginEdit()
    }

    Button {
      visible: root.repeat === "none" && !root.confirmingDelete
      width: Style.space(52)
      height: Style.space(28)
      text: root.strings.t("panel.delete")
      foreground: root.mutedForeground
      fontFamily: root.fontFamily
      horizontalPadding: Style.space(3)
      verticalPadding: 0
      onClicked: root.confirmingDelete = true
    }

    Text {
      visible: root.repeat !== "none"
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: root.strings.t("panel.editRoutinesInApp")
      color: root.mutedForeground
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    Text {
      visible: root.confirmingDelete
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: root.strings.t("panel.confirmDelete")
      color: root.mutedForeground
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    Button {
      visible: root.confirmingDelete
      width: Style.space(32)
      height: Style.space(28)
      text: root.strings.t("panel.yes")
      foreground: root.foreground
      horizontalPadding: 0
      verticalPadding: 0
      onClicked: root.removeConfirmed()
    }

    Button {
      visible: root.confirmingDelete
      width: Style.space(32)
      height: Style.space(28)
      text: root.strings.t("panel.no")
      foreground: root.foreground
      horizontalPadding: 0
      verticalPadding: 0
      onClicked: root.confirmingDelete = false
    }
  }

  Text {
    visible: root.correctionError !== ""
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.bottom
    textFormat: Text.PlainText
    text: root.correctionError
    color: root.mutedForeground
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    elide: Text.ElideRight
  }
}
