import QtQuick
import qs.Commons
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
  required property string measure
  required property int target
  required property int minutes
  required property int count
  required property string repeat
  required property bool timerRunning
  required property string timerStartedAt
  required property int timerMinutes
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

  function focusTimeField() {
    if (!expanded) return
    actions.focusCorrection()
  }

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

  readonly property int elapsedMinutes: {
    host.minuteTick
    host.timerSnapshotMs
    var base = Math.max(0, timerMinutes)
    if (!timerRunning) return base
    return base + Math.max(0, Math.floor((Date.now() - host.timerSnapshotMs) / 60000))
  }
  readonly property var itemData: ({
    "id": itemId, "title": title, "block": block, "state": itemState,
    "measure": measure, "target": target, "minutes": minutes,
    "count": count, "repeat": repeat, "timerRunning": timerRunning,
    "timerStartedAt": timerStartedAt, "timerMinutes": timerMinutes,
    "detailText": detailText, "scheduleText": scheduleText,
    "nextOccurrence": nextOccurrence, "carried": carried
  })

  width: timeline ? timeline.width : 0
  height: rowType === "header" ? Style.space(40)
    : (rowType === "item" ? Style.space(expanded ? (actions.correctionError !== "" ? 102 : 84) : 42)
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
      color: row.done
        ? Style.selectedStateColor(row.foreground, Commons.Color.accent) : "transparent"
      border.width: Style.spacing.hairline
      border.color: row.done
        ? Style.selectedStateColor(row.foreground, Commons.Color.accent) : row.mutedForeground

      Text {
        visible: row.done
        anchors.centerIn: parent
        textFormat: Text.PlainText
        text: row.strings.t("panel.checkSymbol")
        color: Commons.Color.background
        font.family: row.fontFamily
        font.pixelSize: Style.font.caption
      }
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

      Text {
        visible: !row.editing && text !== ""
        width: parent.width
        textFormat: Text.PlainText
        text: row.pending ? row.strings.t("panel.pending") : row.detailText
        color: row.mutedForeground
        font.family: row.fontFamily
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
      }
    }

    Row {
      id: controlRow
      visible: !row.pending && !row.editing
      anchors.right: parent.right
      anchors.rightMargin: Style.space(3)
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(1)

      Row {
        visible: row.measure === "count"
        height: Style.space(26)
        spacing: 0

        Button {
          width: Style.space(24)
          height: parent.height
          text: row.strings.t("panel.decrease")
          tooltipText: row.strings.t("panel.decreaseCount")
          foreground: row.mutedForeground
          fontFamily: row.fontFamily
          horizontalPadding: 0
          verticalPadding: 0
          onClicked: {
            row.selectRow()
            row.host.adjustCount(row.itemData, -1)
          }
        }
        Row {
          width: row.target > 0 ? Style.space(54) : Style.space(32)
          anchors.verticalCenter: parent.verticalCenter
          spacing: Style.space(2)

          Text {
            anchors.verticalCenter: parent.verticalCenter
            textFormat: Text.PlainText
            text: String(row.count)
            color: row.foreground
            font.family: "monospace"
            font.pixelSize: Style.font.caption
          }
          Text {
            visible: row.target > 0
            anchors.verticalCenter: parent.verticalCenter
            textFormat: Text.PlainText
            text: row.strings.t("panel.countGoal").arg(row.target)
            color: row.mutedForeground
            font.family: "monospace"
            font.pixelSize: Style.font.caption
          }
        }
        Button {
          width: Style.space(24)
          height: parent.height
          text: row.strings.t("panel.increase")
          tooltipText: row.strings.t("panel.increaseCount")
          foreground: row.mutedForeground
          fontFamily: row.fontFamily
          horizontalPadding: 0
          verticalPadding: 0
          onClicked: {
            row.selectRow()
            row.host.adjustCount(row.itemData, 1)
          }
        }
      }
      Button {
        width: Style.space(68)
        height: Style.space(26)
        text: (row.timerRunning
            ? row.strings.t("panel.pauseWithTime") : row.strings.t("panel.startWithTime"))
          .arg(row.host.formatMinutes(row.elapsedMinutes))
        tooltipText: row.timerRunning
          ? row.strings.t("panel.pauseTimer") : row.strings.t("panel.startTimer")
        foreground: row.foreground
        fontFamily: "monospace"
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
    host: row.host
    controller: row.controller
    itemData: row.itemData
    rowKey: row.rowKey
    fontFamily: row.fontFamily
    foreground: row.foreground
    mutedForeground: row.mutedForeground
    repeat: row.repeat
    titleEditing: row.editing
    onCorrectMinutes: function(amount) { row.host.correctMinutes(row.itemData, amount) }
    onBeginEdit: row.beginTitleEdit()
    onRemoveConfirmed: {
      row.controller.collapseExpanded()
      row.host.removeItem(row.itemData)
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
