import QtQuick
import qs.Commons
import qs.Ui

// A catalogue can contain many projects. Scroll the chips sideways so its
// size never determines how much of the timeline the sheet occupies.
Row {
  id: root

  required property string caption
  required property var options
  required property var selection
  property bool multiple: false
  required property color foreground
  required property color mutedForeground
  required property string fontFamily
  signal picked(var value)

  spacing: Style.space(5)

  Text {
    id: label
    width: Style.space(54)
    anchors.verticalCenter: parent.verticalCenter
    textFormat: Text.PlainText
    text: root.caption
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    color: root.mutedForeground
    elide: Text.ElideRight
  }

  Flickable {
    id: strip
    width: Math.max(0, root.width - label.width - root.spacing)
    height: chips.implicitHeight
    clip: true
    contentWidth: chips.implicitWidth
    contentHeight: height
    flickableDirection: Flickable.HorizontalFlick
    boundsBehavior: Flickable.StopAtBounds
    interactive: contentWidth > width

    Row {
      id: chips
      spacing: Style.space(3)

      Repeater {
        model: root.options

        Button {
          required property var modelData
          text: modelData.name
          iconText: modelData.color ? " " : ""
          iconSize: Style.font.caption
          leftAlign: true
          width: Math.min(implicitWidth, strip.width)
          selected: root.multiple ? root.selection.includes(modelData.id) : root.selection === modelData.id
          enabled: modelData.enabled !== false
          bordered: true
          foreground: root.foreground
          fontFamily: root.fontFamily
          fontSize: Style.font.bodySmall
          horizontalPadding: Style.space(6)
          verticalPadding: Style.space(2)
          opacity: enabled ? 1 : 0.45
          onClicked: root.picked(modelData.id)

          Rectangle {
            visible: !!modelData.color
            x: parent._reservedContentLeftInset
            anchors.verticalCenter: parent.verticalCenter
            width: Style.space(7)
            height: width
            radius: width / 2
            color: modelData.color || "transparent"
          }
        }
      }
    }
  }
}
