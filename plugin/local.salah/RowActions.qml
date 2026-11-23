import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui

// Expanded-row controls: edit, a two-step delete, "+ Time", and today's
// sessions for the item. Each finished session shows its minutes, which can
// be changed (Enter saves) or removed.
Item {
  id: root

  property var strings
  property var controller
  property string rowKey
  property string fontFamily
  property color foreground
  property color mutedForeground
  property string repeat
  property var sessions: []
  property string runningElapsed: ""
  property string errorText: ""
  property bool confirmingDelete: false

  signal beginEdit()
  signal removeConfirmed()
  signal addTimeRequested()
  signal removeSessionRequested(string sessionId)
  signal minutesEdited(string sessionId, int minutes)

  implicitHeight: column.implicitHeight

  onVisibleChanged: if (!visible) confirmingDelete = false

  // When a timed session started; a logged one has no start time.
  function sessionLabel(session) {
    var started = Date.parse(String(session.started_at || ""))
    if (!isNaN(started)) return Qt.formatTime(new Date(started), "HH:mm")
    var minutes = session.minutes
    return minutes === null || minutes === undefined
      ? root.strings.t("session.tick") : root.strings.t("session.logged")
  }

  // A copy that only changes when the sessions really do, so a status
  // refresh does not rebuild the rows under a half-typed minutes field.
  property var shownSessions: []
  onSessionsChanged: {
    if (JSON.stringify(sessions) !== JSON.stringify(shownSessions)) shownSessions = sessions
  }

  // Keeps the panel's key catcher out of the way while a minutes field has
  // focus (it would otherwise read j/k/h/l as navigation).
  function fieldFocusChanged(focused) {
    if (!root.controller) return
    if (focused) root.controller.editingKey = root.rowKey + ":session"
    else if (root.controller.editingKey === root.rowKey + ":session") root.controller.editingKey = ""
  }

  Column {
    id: column
    anchors.left: parent.left
    anchors.right: parent.right
    spacing: Style.space(3)

    Row {
      height: Style.space(30)
      spacing: Style.space(4)

      Button {
        visible: root.repeat === "none" && !root.confirmingDelete
        anchors.verticalCenter: parent.verticalCenter
        height: Style.space(26)
        text: root.strings.t("panel.edit")
        foreground: root.mutedForeground
        fontFamily: root.fontFamily
        fontSize: Style.font.bodySmall
        horizontalPadding: Style.space(5)
        verticalPadding: 0
        onClicked: root.beginEdit()
      }

      Button {
        visible: root.repeat === "none" && !root.confirmingDelete
        anchors.verticalCenter: parent.verticalCenter
        height: Style.space(26)
        text: root.strings.t("panel.delete")
        foreground: root.mutedForeground
        fontFamily: root.fontFamily
        fontSize: Style.font.bodySmall
        horizontalPadding: Style.space(5)
        verticalPadding: 0
        onClicked: root.confirmingDelete = true
      }

      Text {
        visible: root.repeat !== "none" && !root.confirmingDelete
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
        anchors.verticalCenter: parent.verticalCenter
        width: Style.space(36)
        height: Style.space(26)
        text: root.strings.t("panel.yes")
        foreground: root.foreground
        fontSize: Style.font.bodySmall
        horizontalPadding: 0
        verticalPadding: 0
        onClicked: root.removeConfirmed()
      }

      Button {
        visible: root.confirmingDelete
        anchors.verticalCenter: parent.verticalCenter
        width: Style.space(36)
        height: Style.space(26)
        text: root.strings.t("panel.no")
        foreground: root.foreground
        fontSize: Style.font.bodySmall
        horizontalPadding: 0
        verticalPadding: 0
        onClicked: root.confirmingDelete = false
      }

      Button {
        visible: !root.confirmingDelete
        anchors.verticalCenter: parent.verticalCenter
        height: Style.space(26)
        text: root.strings.t("session.addTime")
        bordered: true
        foreground: root.foreground
        fontFamily: root.fontFamily
        fontSize: Style.font.bodySmall
        horizontalPadding: Style.space(6)
        verticalPadding: 0
        onClicked: root.addTimeRequested()
      }
    }

    Text {
      visible: root.sessions.length > 0
      width: parent.width
      textFormat: Text.PlainText
      text: root.strings.t("session.sessionsToday")
      color: root.mutedForeground
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
    }

    Repeater {
      model: root.shownSessions

      Row {
        id: sessionRow
        required property var modelData
        readonly property bool running: modelData.running === true
        height: Style.space(28)
        spacing: Style.space(5)

        Text {
          anchors.verticalCenter: parent.verticalCenter
          width: Style.space(48)
          textFormat: Text.PlainText
          text: root.sessionLabel(sessionRow.modelData)
          color: root.mutedForeground
          font.family: "monospace"
          font.pixelSize: Style.font.caption
        }

        Text {
          visible: sessionRow.running
          anchors.verticalCenter: parent.verticalCenter
          textFormat: Text.PlainText
          text: root.strings.t("session.running") + "  " + root.runningElapsed
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
        }

        TextField {
          visible: !sessionRow.running
          anchors.verticalCenter: parent.verticalCenter
          width: Style.space(54)
          height: Style.space(24)
          text: sessionRow.modelData.minutes === null || sessionRow.modelData.minutes === undefined
            ? "" : String(sessionRow.modelData.minutes)
          placeholderText: "—"
          foreground: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          horizontalPadding: Style.space(5)
          verticalPadding: 0
          maximumLength: 4
          inputMethodHints: Qt.ImhDigitsOnly
          validator: IntValidator { bottom: 1; top: 1440 }
          onActiveFocusChanged: root.fieldFocusChanged(activeFocus)
          onAccepted: {
            var minutes = Number(text)
            if (minutes !== Number(sessionRow.modelData.minutes))
              root.minutesEdited(String(sessionRow.modelData.id), minutes)
            focus = false
          }
          Keys.onEscapePressed: function(event) {
            text = sessionRow.modelData.minutes === null || sessionRow.modelData.minutes === undefined
              ? "" : String(sessionRow.modelData.minutes)
            focus = false
            event.accepted = true
          }
        }

        Text {
          visible: !sessionRow.running
          anchors.verticalCenter: parent.verticalCenter
          textFormat: Text.PlainText
          text: root.strings.t("session.minutesUnit")
          color: root.mutedForeground
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }

        Button {
          visible: !sessionRow.running
          anchors.verticalCenter: parent.verticalCenter
          width: Style.space(26)
          height: Style.space(24)
          text: root.strings.t("session.remove")
          tooltipText: root.strings.t("session.removeTooltip")
          foreground: root.mutedForeground
          fontFamily: root.fontFamily
          horizontalPadding: 0
          verticalPadding: 0
          onClicked: root.removeSessionRequested(String(sessionRow.modelData.id))
        }
      }
    }

    Text {
      visible: root.errorText !== ""
      width: parent.width
      textFormat: Text.PlainText
      text: root.errorText
      color: Commons.Color.urgent
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      wrapMode: Text.Wrap
      maximumLineCount: 2
      elide: Text.ElideRight
    }
  }
}
