import QtQuick
import qs.Commons
import qs.Ui

// Expanded-row controls: minute correction, edit, and a two-step delete.
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
  property string correctionError: ""

  signal correctMinutes(int amount)
  signal beginEdit()
  signal removeConfirmed()

  height: Style.space(38)

  function focusCorrection() {
    correctionField.forceActiveFocus()
    correctionField.selectAll()
  }

  onVisibleChanged: if (!visible) {
    confirmingDelete = false
    correctionError = ""
    correctionField.text = ""
  }

  property bool confirmingDelete: false

  Row {
    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    spacing: Style.space(4)

    Text {
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: root.strings.t("panel.time")
      color: root.mutedForeground
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    TextField {
      id: correctionField
      width: Style.space(78)
      height: Style.space(28)
      placeholderText: root.strings.t("panel.correctionPlaceholder")
      foreground: root.foreground
      font.family: "monospace"
      font.pixelSize: Style.font.caption
      horizontalPadding: Style.space(5)
      verticalPadding: 0
      maximumLength: 5
      onActiveFocusChanged: {
        if (activeFocus) root.controller.editingKey = root.rowKey
        else if (root.controller.editingKey === root.rowKey && !root.titleEditing)
          root.controller.editingKey = ""
      }
      onAccepted: {
        var raw = String(text).trim()
        var value = Number(raw)
        if (!/^[+-]?\d+$/.test(raw) || value === 0 || value < -1440 || value > 1440) {
          root.correctionError = root.strings.t("panel.correctionError")
          return
        }
        root.correctMinutes(value)
        text = ""
        root.correctionError = ""
      }
      Keys.onEscapePressed: function(event) {
        text = ""
        root.correctionError = ""
        root.controller.collapseExpanded()
        root.controller.forceActiveFocus()
        event.accepted = true
      }
    }

    Repeater {
      model: [
        { "amount": -5, "key": "panel.minusFive" },
        { "amount": 5, "key": "panel.plusFive" },
        { "amount": 15, "key": "panel.plusFifteen" }
      ]
      Button {
        required property var modelData
        width: Style.space(34)
        height: Style.space(28)
        text: root.strings.t(modelData.key)
        foreground: root.mutedForeground
        fontFamily: "monospace"
        horizontalPadding: 0
        verticalPadding: 0
        onClicked: root.correctMinutes(modelData.amount)
      }
    }

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
