import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui

Column {
  id: due

  required property var host
  required property string fontFamily
  spacing: Style.space(4)
  visible: host.dueReminders.length > 0 || host.reminderError !== ""

  PanelSectionHeader {
    width: parent.width
    text: due.host.tr.t("reminder.due")
    foreground: due.host.foreground
    fontFamily: due.fontFamily
  }

  ListView {
    id: reminderList
    width: parent.width
    height: Math.min(count, 3) * Style.space(34)
    model: due.host.dueReminders
    clip: true
    interactive: count > 3
    boundsBehavior: Flickable.StopAtBounds

    delegate: Item {
      id: row
      required property var modelData
      readonly property string subject: modelData.item_title || modelData.epic_name || ""
      readonly property string time: due.host.resourceDateTime(modelData.remind_at)
      width: reminderList.width
      height: Style.space(34)

      Column {
        anchors.left: parent.left
        anchors.right: tick.left
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: row.modelData.title || row.subject || due.host.tr.t("reminder.untitled")
          color: due.host.foreground
          font.family: due.fontFamily
          font.pixelSize: Style.font.bodySmall
          elide: Text.ElideRight
        }

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: row.modelData.title && row.subject
            ? due.host.tr.t("reminder.subjectTime").arg(row.subject).arg(row.time) : row.time
          color: due.host.mutedForeground
          font.family: due.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }
      }

      Button {
        id: tick
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: Style.space(30)
        height: Style.space(25)
        enabled: !due.host.pendingReminders[row.modelData.id]
        opacity: enabled ? 1 : 0.45
        text: due.host.tr.t("panel.checkSymbol")
        tooltipText: due.host.tr.t("reminder.markDone")
        bordered: true
        foreground: due.host.foreground
        fontFamily: due.fontFamily
        horizontalPadding: 0
        verticalPadding: 0
        onClicked: due.host.completeReminder(row.modelData.id)
      }
    }
  }

  Text {
    visible: text !== ""
    width: parent.width
    textFormat: Text.PlainText
    text: due.host.reminderError
    color: Commons.Color.urgent
    font.family: due.fontFamily
    font.pixelSize: Style.font.caption
    wrapMode: Text.WrapAnywhere
    maximumLineCount: 2
    elide: Text.ElideRight
  }
}
