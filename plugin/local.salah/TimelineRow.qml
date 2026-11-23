import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui

// One row in the day timeline: a prayer header, an item, an add row, or a
// boundary marker. Drag, expand, and edit state live in the panel controller
// passed in as `controller`.
Item {
  id: row

  required property int index
  required property string rowType
  required property string rowKey
  required property string block
  required property string itemId
  required property string title
  required property string time
  required property string itemState
  required property string projectId
  required property color projectColor
  required property int plannedMinutes
  required property int minutes
  required property int count
  required property string repeat
  required property bool timerRunning
  required property string detailText
  required property string scheduleText
  required property string nextOccurrence
  required property bool carried
  required property bool current
  required property bool marked
  required property bool passed
  required property bool pending

  property var host
  property var controller
  property var timeline
  property var strings
  property string fontFamily
  property color foreground
  property color mutedForeground

  readonly property bool dragging: controller !== null && controller.dragKey === rowKey
  readonly property bool done: itemState === "done"
  readonly property bool expanded: rowType === "item"
    && controller !== null && controller.expandedItemId === itemId
  property bool editing: false
  property string editText: title

  onExpandedChanged: if (!expanded) editing = false

  function selectRow() {
    timeline.currentIndex = index
    if (controller.expandedItemId !== ""
        && controller.expandedItemId !== itemId)
      controller.collapseExpanded()
  }

  function beginTitleEdit() {
    if (repeat !== "none") return
    editing = true
    editText = title
    controller.editingKey = rowKey
    titleEditor.forceActiveFocus()
    titleEditor.selectAll()
  }

  readonly property var itemData: ({
    "id": itemId, "title": title, "block": block, "state": itemState,
    "projectId": projectId, "projectColor": projectColor, "plannedMinutes": plannedMinutes, "minutes": minutes,
    "count": count, "repeat": repeat, "timerRunning": timerRunning,
    "detailText": detailText, "scheduleText": scheduleText,
    "nextOccurrence": nextOccurrence, "carried": carried
  })
  // Today's sessions of this item, read from the latest status report.
  readonly property var sessions: rowType === "item" && expanded && host
    ? host.itemSessions(itemId, host.report) : []

  width: timeline ? timeline.width : 0
  height: rowType === "header" ? Style.space(40)
    : (rowType === "item" ? (expanded ? Style.space(48) + actions.implicitHeight : Style.space(42))
      : (rowType === "marker" ? Style.space(24) : Style.space(27)))
  Behavior on height {
    NumberAnimation { duration: Style.duration(120); easing.type: Easing.OutCubic }
  }
  opacity: dragging ? 0 : (pending ? 0.5 : (passed && !current ? 0.58 : 1))

  PrayerRow {
    anchors.fill: parent
    visible: row.rowType === "header"
    strings: row.strings
    fontFamily: row.fontFamily
    foreground: row.foreground
    mutedForeground: row.mutedForeground
    block: row.block
    title: row.host ? row.host.prayerName(row.block, row.title) : row.title
    time: row.time
    current: row.current
    marked: row.marked
    onToggleRequested: row.host.togglePrayer({ "id": row.block })
  }

  Rectangle {
    visible: row.rowType === "item" && row.expanded
    anchors.fill: parent
    anchors.leftMargin: Style.space(28)
    radius: Style.cornerRadius
    color: Style.hoverFillFor(row.foreground, Commons.Color.accent)
    border.width: Style.spacing.hairline
    border.color: Style.normalBorderFor(row.foreground, Commons.Color.accent)
  }

  Rectangle {
    visible: row.rowType === "item"
    anchors.top: parent.top
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: Style.space(28)
    height: Style.space(42)
    radius: Style.cornerRadius
    color: rowHover.containsMouse && !row.expanded
      ? Style.hoverFillFor(row.foreground, Commons.Color.accent) : "transparent"

    MouseArea {
      id: rowHover
      anchors.fill: parent
      hoverEnabled: true
      acceptedButtons: Qt.NoButton
    }

    Text {
      id: dragGlyph
      anchors.left: parent.left
      anchors.leftMargin: Style.space(4)
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: row.strings.t("panel.dragHandle")
      color: row.mutedForeground
      font.family: row.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    MouseArea {
      id: handleMouse
      anchors.left: parent.left
      anchors.top: parent.top
      anchors.bottom: parent.bottom
      width: Style.space(28)
      cursorShape: Qt.SizeVerCursor
      preventStealing: true
      property bool activeDrag: false
      onPressed: function(mouse) {
        activeDrag = true
        row.selectRow()
        var point = mapToItem(row.timeline.contentItem, mouse.x, mouse.y)
        row.controller.beginDrag(row.rowKey, row.title, point.y)
      }
      onPositionChanged: function(mouse) {
        if (!activeDrag) return
        var point = mapToItem(row.timeline.contentItem, mouse.x, mouse.y)
        row.controller.moveDrag(point.y)
      }
      onReleased: function(mouse) {
        var point = mapToItem(row.timeline.contentItem, mouse.x, mouse.y)
        var inside = point.x >= 0 && point.x <= row.timeline.width
          && point.y >= 0 && point.y <= row.timeline.contentHeight
        activeDrag = false
        row.controller.finishDrag(inside)
      }
      onCanceled: { activeDrag = false; row.controller.cancelDrag() }
    }

    Rectangle {
      id: itemCheck
      anchors.left: parent.left
      anchors.leftMargin: Style.space(30)
      anchors.verticalCenter: parent.verticalCenter
      width: Style.space(15)
      height: width
      radius: Math.min(Style.cornerRadius, Style.space(3))
      // Done is a translucent selected fill with the check in the
      // foreground colour: no light block on the dark popup.
      color: row.done
        ? Style.selectedFillFor(row.foreground, Commons.Color.accent) : "transparent"
      border.width: Style.spacing.hairline
      border.color: row.done
        ? Style.selectedBorderFor(row.foreground, Commons.Color.accent) : row.mutedForeground

      Text {
        visible: row.done
        anchors.centerIn: parent
        textFormat: Text.PlainText
        text: row.strings.t("panel.checkSymbol")
        color: row.foreground
        font.family: row.fontFamily
        font.pixelSize: Style.font.caption
      }
    }

    MouseArea {
      anchors.fill: itemCheck
      cursorShape: Qt.PointingHandCursor
      onClicked: if (!row.pending) row.host.toggleItem(row.itemData)
    }

    MouseArea {
      id: itemMouse
      anchors.left: itemCheck.left
      anchors.right: controlRow.left
      anchors.top: parent.top
      anchors.bottom: parent.bottom
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      pressAndHoldInterval: 250
      property bool draggedThisPress: false
      onPressed: {
        draggedThisPress = false
        row.timeline.currentIndex = row.index
        row.forceActiveFocus()
        if (row.controller.expandedItemId !== ""
            && row.controller.expandedItemId !== row.itemId)
          row.controller.collapseExpanded()
      }
      onPressAndHold: function(mouse) {
        draggedThisPress = true
        var point = mapToItem(row.timeline.contentItem, mouse.x, mouse.y)
        row.controller.beginDrag(row.rowKey, row.title, point.y)
      }
      onPositionChanged: function(mouse) {
        if (!draggedThisPress) return
        var point = mapToItem(row.timeline.contentItem, mouse.x, mouse.y)
        row.controller.moveDrag(point.y)
      }
      onReleased: function(mouse) {
        if (!draggedThisPress) return
        var point = mapToItem(row.timeline.contentItem, mouse.x, mouse.y)
        var inside = point.x >= 0 && point.x <= row.timeline.width
          && point.y >= 0 && point.y <= row.timeline.contentHeight
        row.controller.finishDrag(inside)
      }
      onCanceled: if (draggedThisPress) row.controller.cancelDrag()
      onClicked: if (!draggedThisPress && !row.pending) row.host.toggleItem(row.itemData)
    }

    Column {
      anchors.left: itemCheck.right
      anchors.leftMargin: Style.space(9)
      anchors.right: controlRow.left
      anchors.rightMargin: Style.space(6)
      anchors.verticalCenter: parent.verticalCenter
      spacing: 0

      Text {
        visible: !row.editing
        width: parent.width
        textFormat: Text.PlainText
        text: row.title
        color: row.done || row.pending ? row.mutedForeground : row.foreground
        font.family: row.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.strikeout: row.done
        elide: Text.ElideRight
      }

      TextField {
        id: titleEditor
        visible: row.editing
        width: parent.width
        height: Style.space(26)
        foreground: row.foreground
        font.family: row.fontFamily
        font.pixelSize: Style.font.bodySmall
        horizontalPadding: Style.space(4)
        verticalPadding: 0
        maximumLength: 200
        text: row.editText
        onAccepted: {
          row.host.editItem(row.itemData, text)
          row.editing = false
          row.controller.editingKey = ""
          row.controller.forceActiveFocus()
        }
        Keys.onEscapePressed: function(event) {
          row.editing = false
          row.controller.editingKey = ""
          row.controller.forceActiveFocus()
          event.accepted = true
        }
      }

      Row {
        visible: !row.editing && detail.text !== ""
        width: parent.width
        spacing: Style.space(4)

        Rectangle {
          visible: row.projectId !== ""
          anchors.verticalCenter: parent.verticalCenter
          width: Style.space(6)
          height: width
          radius: width / 2
          color: row.projectColor
        }

        Text {
          id: detail
          width: parent.width - (row.projectId !== "" ? Style.space(10) : 0)
          textFormat: Text.PlainText
          text: row.pending ? row.strings.t("panel.pending") : row.detailText
          color: row.mutedForeground
          font.family: row.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }
      }
    }

    Row {
      id: controlRow
      visible: !row.pending && !row.editing
      anchors.right: parent.right
      anchors.rightMargin: Style.space(3)
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(1)

      Text {
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: row.plannedMinutes > 0 ? row.plannedMinutes + "m" : ""
        color: row.mutedForeground
        font.family: "monospace"
        font.pixelSize: Style.font.caption
      }
      Button {
        width: row.timerRunning ? Style.space(68) : Style.space(32)
        height: Style.space(26)
        text: row.timerRunning
          ? row.strings.t("panel.stopWithTime").arg(row.host.runningElapsedText)
          : row.strings.t("panel.startSymbol")
        tooltipText: row.timerRunning
          ? row.strings.t("panel.stopTimer") : row.strings.t("panel.startTimer")
        foreground: row.foreground
        fontFamily: row.fontFamily
        fontSize: Style.font.bodySmall
        selected: row.timerRunning
        horizontalPadding: 0
        verticalPadding: 0
        bordered: true
        onClicked: {
          row.selectRow()
          row.host.toggleTimer(row.itemData)
        }
      }
      Button {
        width: Style.space(30)
        height: Style.space(28)
        text: row.strings.t("panel.moreSymbol")
        tooltipText: row.strings.t("panel.moreActions")
        foreground: row.mutedForeground
        fontFamily: row.fontFamily
        horizontalPadding: 0
        verticalPadding: 0
        onClicked: {
          row.timeline.currentIndex = row.index
          row.controller.toggleExpanded(row.itemId)
        }
      }
    }
  }

  RowActions {
    id: actions
    visible: row.rowType === "item" && row.expanded
    anchors.left: parent.left
    anchors.leftMargin: Style.space(58)
    anchors.right: parent.right
    anchors.rightMargin: Style.space(3)
    anchors.top: parent.top
    anchors.topMargin: Style.space(44)
    strings: row.strings
    controller: row.controller
    rowKey: row.rowKey
    fontFamily: row.fontFamily
    foreground: row.foreground
    mutedForeground: row.mutedForeground
    repeat: row.repeat
    sessions: row.sessions
    runningElapsed: row.host ? row.host.runningElapsedText : ""
    errorText: row.host && row.host.rowErrorKey === row.rowKey ? row.host.rowErrorText : ""
    onBeginEdit: row.beginTitleEdit()
    onRemoveConfirmed: {
      row.controller.collapseExpanded()
      row.host.removeItem(row.itemData)
    }
    onAddTimeRequested: row.host.openLogPanel("item:" + row.itemId)
    onRemoveSessionRequested: function(sessionId) { row.host.removeSession(row.itemData, sessionId) }
    onMinutesEdited: function(sessionId, minutes) {
      row.host.editSessionMinutes(row.itemData, sessionId, minutes)
    }
  }

  Button {
    visible: row.rowType === "add"
    anchors.left: parent.left
    anchors.leftMargin: Style.space(34)
    anchors.verticalCenter: parent.verticalCenter
    height: Style.space(25)
    text: row.strings.t("panel.addRow").arg(row.host.prayerName(row.block, row.block))
    tooltipText: row.strings.t("panel.addTo").arg(row.host.prayerName(row.block, row.block))
    foreground: row.mutedForeground
    fontFamily: row.fontFamily
    horizontalPadding: Style.space(5)
    verticalPadding: Style.space(1)
    onClicked: row.host.presetQuickAdd(row.block)
  }

  Item {
    visible: row.rowType === "marker"
    anchors.fill: parent

    Text {
      anchors.left: parent.left
      anchors.leftMargin: Style.space(38)
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: row.rowKey === "marker:sunrise"
        ? row.strings.t("boundary.sunrise") : row.strings.t("boundary.sleep")
      color: row.mutedForeground
      font.family: row.fontFamily
      font.pixelSize: Style.font.caption
    }
    Text {
      anchors.right: parent.right
      anchors.rightMargin: Style.space(10)
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: row.time
      color: row.mutedForeground
      font.family: row.fontFamily
      font.pixelSize: Style.font.caption
    }
  }
}
