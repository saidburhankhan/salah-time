import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "SessionLogic.js" as SessionLogic

// One metric prompt: a label, then a control that suits its kind. A scale is
// a row of 1..scale_max tap buttons (tapping the chosen one clears it), a
// count is a number field with its unit, and a check is a switch.
Item {
  id: field

  property var strings
  property var metric: ({})
  property var value: undefined
  property var storedValue: undefined
  property string fontFamily
  property color foreground
  property color mutedForeground
  property real labelWidth: Style.space(96)
  readonly property bool textFocus: countInput.activeFocus
  readonly property string kind: String(metric && metric.kind || "")
  readonly property string metricId: String(metric && metric.metric_id || "")
  readonly property int scaleMax: Math.max(1, Math.min(10, Number(metric && metric.scale_max) || 5))
  readonly property bool hasValue: value !== undefined && value !== null

  signal valueEdited(var value)
  signal escapePressed()
  signal accepted()

  function reset() {
    countInput.edited = false
    countInput.text = storedValue === undefined || storedValue === null ? "" : String(storedValue)
  }

  onStoredValueChanged: {
    if (countInput && !countInput.edited) reset()
  }

  implicitHeight: Math.max(label.implicitHeight, controls.implicitHeight)

  Component.onCompleted: {
    if (kind === "count" && hasValue) {
      countInput.edited = value !== storedValue
      countInput.text = String(value)
    }
  }

  Text {
    id: label
    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    width: field.labelWidth
    textFormat: Text.PlainText
    text: String(field.metric && field.metric.name || "")
    color: field.foreground
    font.family: field.fontFamily
    font.pixelSize: Style.font.bodySmall
    elide: Text.ElideRight
  }

  Row {
    id: controls
    anchors.left: label.right
    anchors.leftMargin: Style.space(6)
    anchors.verticalCenter: parent.verticalCenter
    spacing: Style.space(3)

    Repeater {
      model: field.kind === "scale" ? field.scaleMax : 0

      Button {
        required property int index
        width: Style.space(28)
        height: Style.space(24)
        text: String(index + 1)
        // Selected = the theme's translucent selected fill and bold label:
        // a dark raised tone, never a light block.
        selected: field.hasValue && Number(field.value) === index + 1
        bordered: true
        foreground: field.foreground
        fontFamily: field.fontFamily
        fontSize: Style.font.bodySmall
        horizontalPadding: 0
        verticalPadding: 0
        onClicked: field.valueEdited(selected ? null : index + 1)
      }
    }

    TextField {
      id: countInput
      property bool edited: false
      visible: field.kind === "count"
      width: Style.space(70)
      height: Style.space(26)
      placeholderText: "0"
      foreground: field.foreground
      font.family: field.fontFamily
      font.pixelSize: Style.font.bodySmall
      horizontalPadding: Style.space(5)
      verticalPadding: 0
      maximumLength: 9
      inputMethodHints: Qt.ImhFormattedNumbersOnly
      onTextEdited: {
        edited = true
        field.valueEdited(SessionLogic.parseCount(text))
      }
      onAccepted: field.accepted()
      Keys.onEscapePressed: function(event) {
        field.escapePressed()
        event.accepted = true
      }
    }

    Text {
      visible: field.kind === "count" && String(field.metric && field.metric.unit || "") !== ""
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: String(field.metric && field.metric.unit || "")
      color: field.mutedForeground
      font.family: field.fontFamily
      font.pixelSize: Style.font.caption
    }

    ToggleSwitch {
      visible: field.kind === "check"
      anchors.verticalCenter: parent.verticalCenter
      checked: field.hasValue && Number(field.value) === 1
      foreground: field.foreground
      cursorRing: false
      onToggled: field.valueEdited(checked ? 0 : 1)
    }

    Text {
      visible: field.kind === "check"
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: field.hasValue && Number(field.value) === 1
        ? field.strings.t("session.yes") : field.strings.t("session.no")
      color: field.mutedForeground
      font.family: field.fontFamily
      font.pixelSize: Style.font.caption
    }
  }
}
