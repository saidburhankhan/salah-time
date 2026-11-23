import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "SessionLogic.js" as SessionLogic

Rectangle {
  id: card

  required property var host
  readonly property string fontFamily: host.bar ? host.bar.fontFamily : Style.font.family
  readonly property alias noteFocus: editor.textFocus
  readonly property var timer: host.runningTimer
  readonly property var sleepCap: timer && timer.stale === true
    ? SessionLogic.sessionMinuteCap(timer.max_minutes) : null

  implicitHeight: content.implicitHeight + Style.space(10)
  radius: Style.cornerRadius
  color: Style.hoverFillFor(host.foreground, Commons.Color.accent)
  border.width: Style.spacing.hairline
  border.color: Style.normalBorderFor(host.foreground, Commons.Color.accent)

  Column {
    id: content
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.topMargin: Style.space(4)
    spacing: Style.space(3)

    Item {
      width: parent.width
      height: Style.space(32)

      Rectangle {
        id: runningDot
        anchors.left: parent.left
        anchors.leftMargin: Style.space(10)
        anchors.verticalCenter: parent.verticalCenter
        width: Style.space(9)
        height: width
        radius: width / 2
        color: card.host.runningColor
      }

      Text {
        anchors.left: runningDot.right
        anchors.leftMargin: Style.space(8)
        anchors.right: runningElapsed.left
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: card.host.runningTitle
        color: card.host.foreground
        font.family: card.fontFamily
        font.pixelSize: Style.font.body
        elide: Text.ElideRight
      }

      Text {
        id: runningElapsed
        anchors.right: stopButton.left
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: card.host.runningElapsedText
        color: card.host.foreground
        font.family: card.fontFamily
        // Tabular digits keep the ticking clock the same width.
        font.features: { "tnum": 1 }
        font.pixelSize: Style.font.body
        font.bold: true
      }

      Button {
        id: stopButton
        anchors.right: parent.right
        anchors.rightMargin: Style.space(6)
        anchors.verticalCenter: parent.verticalCenter
        height: Style.space(28)
        text: card.host.tr.t("session.stop")
        tooltipText: card.host.tr.t("session.stopTooltip")
        bordered: true
        foreground: card.host.foreground
        fontFamily: card.fontFamily
        fontSize: Style.font.bodySmall
        horizontalPadding: Style.space(8)
        verticalPadding: 0
        onClicked: card.host.pauseRunning()
      }
    }

    Text {
      visible: card.sleepCap !== null
      x: Style.space(10)
      width: parent.width - x * 2
      textFormat: Text.PlainText
      text: visible ? card.host.tr.t("session.ranPastSleep").arg(card.sleepCap) : ""
      color: card.host.mutedForeground
      font.family: card.fontFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
    }

    Row {
      x: Style.space(10)
      spacing: Style.space(5)

      Text {
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: card.host.tr.t("session.distractions").arg(card.host.distractions)
        color: card.host.mutedForeground
        font.family: card.fontFamily
        font.pixelSize: Style.font.bodySmall
      }

      Repeater {
        model: [-1, 1]

        Button {
          required property int modelData
          width: Style.space(26)
          height: Style.space(24)
          enabled: card.host.canAnnotateTimer
          opacity: enabled ? 1 : 0.45
          text: modelData < 0 ? "−" : "+"
          tooltipText: card.host.tr.t(modelData < 0 ? "panel.decreaseCount" : "panel.increaseCount")
          bordered: true
          foreground: card.host.foreground
          fontFamily: card.fontFamily
          horizontalPadding: 0
          verticalPadding: 0
          onClicked: card.host.bump(modelData, false)
        }
      }

      Button {
        height: Style.space(24)
        enabled: card.host.canAnnotateTimer
        opacity: enabled ? 1 : 0.45
        text: card.host.tr.t("session.addNote")
        bordered: true
        foreground: card.host.foreground
        fontFamily: card.fontFamily
        fontSize: Style.font.bodySmall
        verticalPadding: 0
        onClicked: {
          card.host.openNote("card")
          editor.focusInput()
        }
      }
    }

    SessionNoteField {
      id: editor
      x: Style.space(10)
      width: parent.width - x * 2
      visible: card.host.noteSource === "card"
      host: card.host
    }
  }
}
