import QtQuick
import qs.Commons
import qs.Ui

// One prayer/block header row with its mark circle, time, and NOW tag.
Item {
  id: root

  property var strings
  property string fontFamily
  property color foreground
  property color mutedForeground
  property string block
  property string title
  property string time
  property bool current
  property bool marked

  signal toggleRequested()

  width: parent ? parent.width : 0
  height: Style.space(40)

  Rectangle {
    anchors.fill: parent
    radius: Style.cornerRadius
    color: root.current
      ? Style.selectedFillFor(root.foreground, Commons.Color.accent)
      : (headerHover.containsMouse
          ? Style.hoverFillFor(root.foreground, Commons.Color.accent)
          : "transparent")

    Rectangle {
      id: prayerCircle
      visible: root.block !== "night"
      anchors.left: parent.left
      anchors.leftMargin: Style.space(8)
      anchors.verticalCenter: parent.verticalCenter
      width: Style.space(18)
      height: width
      radius: width / 2
      color: root.marked
        ? Style.selectedStateColor(root.foreground, Commons.Color.accent) : "transparent"
      border.width: Style.spacing.hairline
      border.color: root.marked
        ? Style.selectedStateColor(root.foreground, Commons.Color.accent) : root.foreground

      Text {
        visible: root.marked
        anchors.centerIn: parent
        textFormat: Text.PlainText
        text: root.strings.t("panel.checkSymbol")
        color: Commons.Color.background
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }

      MouseArea {
        anchors.fill: parent
        anchors.margins: -Style.space(6)
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggleRequested()
      }
    }

    MouseArea {
      id: headerHover
      anchors.left: root.block === "night" ? parent.left : prayerCircle.right
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.bottom: parent.bottom
      hoverEnabled: true
      acceptedButtons: Qt.NoButton
    }

    Text {
      anchors.left: parent.left
      anchors.leftMargin: root.block === "night" ? Style.space(8) : Style.space(36)
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: root.title
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      font.bold: root.current
    }

    Text {
      anchors.right: nowTag.left
      anchors.rightMargin: Style.space(8)
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: root.time || root.strings.t("panel.noTimeSymbol")
      color: root.mutedForeground
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    Rectangle {
      id: nowTag
      visible: root.current
      anchors.right: parent.right
      anchors.rightMargin: Style.space(6)
      anchors.verticalCenter: parent.verticalCenter
      width: visible ? nowText.implicitWidth + Style.space(12) : 0
      height: Style.space(22)
      radius: Style.cornerRadius
      color: Style.selectedFillFor(root.foreground, Commons.Color.accent)

      Text {
        id: nowText
        anchors.centerIn: parent
        textFormat: Text.PlainText
        text: root.strings.t("panel.now")
        color: Style.selectedStateColor(root.foreground, Commons.Color.accent)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }
    }
  }
}
