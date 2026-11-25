import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "SessionLogic.js" as SessionLogic

// Expanded-row controls: edit, a two-step delete, "+ Time", and today's
// sessions for the item. Each finished session shows its minutes, which can
// be changed (Enter saves) or removed.
Item {
  id: root

  property var strings
  property var controller
  property string rowKey
  property var resourceHost
  property string resourceItemId
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

  function cancelForm() {
    if (confirmingDelete) {
      confirmingDelete = false
      return true
    }
    return resources.item && resources.item.cancelDraft()
  }

  function saveForm() {
    if (confirmingDelete) {
      removeConfirmed()
      return true
    }
    if (!resources.item || resources.item.addingKind === "") return false
    resources.item.save()
    return true
  }

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
  readonly property var sessionIds: sessions.map(session => session.id)
  property var shownIds: []
  onSessionIdsChanged: {
    var source = root.resourceHost && root.resourceHost.noteSource
    if (source && source.indexOf("row:") === 0
        && shownIds.some(id => source.indexOf("row:" + id + ":") === 0 && !sessionIds.includes(id)))
      root.resourceHost.closeNote()
    if (JSON.stringify(sessionIds) !== JSON.stringify(shownIds)) shownIds = sessionIds
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
        onClicked: {
          if (!root.controller.leaveRowNote()) return
          root.confirmingDelete = true
          root.controller.forceActiveFocus()
        }
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

    Loader {
      id: resources
      active: root.visible && root.resourceItemId !== ""
      width: parent.width
      sourceComponent: ItemResources {
        host: root.resourceHost
        itemId: root.resourceItemId
        fontFamily: root.fontFamily
        onTextFocusChanged: root.fieldFocusChanged(textFocus)
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
      model: root.shownIds

      Column {
        id: sessionRow
        required property var modelData
        readonly property var session: root.sessions.find(row => row.id === modelData) || null
        readonly property bool running: session && session.running === true
        readonly property bool editable: SessionLogic.canEditMinutes(session)
        visible: session !== null
        width: column.width

        Row {
          height: Style.space(28)
          spacing: Style.space(5)

          Text {
            anchors.verticalCenter: parent.verticalCenter
            width: Style.space(48)
            textFormat: Text.PlainText
            text: sessionRow.session ? root.sessionLabel(sessionRow.session) : ""
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
            readonly property string savedText: sessionRow.editable ? String(sessionRow.session.minutes) : ""
            visible: sessionRow.editable
            anchors.verticalCenter: parent.verticalCenter
            width: Style.space(54)
            height: Style.space(24)
            foreground: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
            horizontalPadding: Style.space(5)
            verticalPadding: 0
            maximumLength: 4
            inputMethodHints: Qt.ImhDigitsOnly
            validator: IntValidator { bottom: 1; top: 1440 }
            Component.onCompleted: text = savedText
            onSavedTextChanged: if (!activeFocus) text = savedText
            onActiveFocusChanged: {
              if (!activeFocus) text = savedText
              root.fieldFocusChanged(activeFocus)
            }
            onAccepted: {
              var minutes = Number(text)
              if (sessionRow.editable && minutes !== Number(sessionRow.session.minutes))
                root.minutesEdited(String(sessionRow.session.id), minutes)
              root.controller.forceActiveFocus()
            }
            Keys.onEscapePressed: function(event) {
              root.controller.forceActiveFocus()
              event.accepted = true
            }
          }

          Text {
            visible: sessionRow.editable
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
            onClicked: if (sessionRow.session) root.removeSessionRequested(String(sessionRow.session.id))
          }
        }

        SessionNotes {
          width: parent.width
          host: root.resourceHost
          session: sessionRow.session
          surface: "row"
          visible: implicitHeight > 0
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
