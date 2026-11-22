import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "DragLogic.js" as DragLogic

BarWidget {
  id: root
  moduleName: "local.salah"

  Commons.Translations {
    id: strings
    directory: Qt.resolvedUrl("i18n")
  }

  readonly property var tr: strings
  property var report: null
  property double countdownAtMs: 0
  property double countdownNowMs: Date.now()
  readonly property int remainingSeconds: {
    countdownNowMs
    if (!(countdownAtMs > 0)) return 0
    return Math.max(0, Math.floor((countdownAtMs - countdownNowMs) / 1000))
  }
  property string errorMessage: ""
  property string locationWarning: ""
  property string officialWarning: ""
  property bool locationSyncPendingStatus: false
  property bool autoLocation: true
  property string lastLocationSync: ""
  property bool firstLoad: true
  property bool popoutSwitchClosing: false
  property string actionError: ""
  property string quickAddBlock: ""
  property int minuteTick: 0
  property var pendingOps: ({})
  property double timerSnapshotMs: Date.now()

  readonly property string nextName: report && report.next
    ? String(report.next.name || "")
    : ""
  readonly property string localizedNextName: report && report.next
    ? prayerName(report.next.id, nextName)
    : ""
  readonly property string nextTime: report && report.next
    ? String(report.next.time || "")
    : ""
  readonly property string remainingLong: formatRemaining(remainingSeconds, false)
  readonly property string remainingShort: formatRemaining(remainingSeconds, true)
  readonly property var planner: report && report.planner ? report.planner : null
  readonly property var runningTimer: {
    var rows = planner && planner.items ? planner.items : []
    for (var i = 0; i < rows.length; i++) {
      var timer = rows[i].timer
      if (timer && timer.running === true) return {
        "item_id": String(rows[i].id || ""),
        "title": String(rows[i].title || ""),
        "started_at": String(timer.started_at || ""),
        "minutes": Number(timer.minutes_including_running) || 0
      }
    }
    return planner && planner.timer ? planner.timer : null
  }
  readonly property int runningMinutes: {
    minuteTick
    timerSnapshotMs
    if (!runningTimer) return 0
    var base = Math.max(0, Number(runningTimer.minutes) || 0)
    return base + Math.max(0, Math.floor((Date.now() - timerSnapshotMs) / 60000))
  }
  readonly property string displayText: localizedNextName !== ""
    ? strings.t("bar.nextStatus").arg(localizedNextName).arg(remainingShort)
        .arg(runningTimer ? strings.t("bar.runningTimer").arg(formatMinutes(runningMinutes)) : "")
    : (firstLoad ? strings.t("bar.loading") : strings.t("bar.unavailableShort"))
  readonly property color foreground: bar ? bar.foreground : Commons.Color.foreground
  readonly property color mutedForeground: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.58)
  readonly property bool opened: panel.open
  readonly property real openPanelIndicatorWidth: button.labelWidth

  function formatRemaining(total, compact) {
    var seconds = Math.max(0, Math.floor(Number(total) || 0))
    var hours = Math.floor(seconds / 3600)
    var minutes = Math.floor((seconds % 3600) / 60)
    var secs = seconds % 60
    function pad(value) { return value < 10 ? "0" + value : String(value) }
    if (compact) {
      if (hours > 0) return strings.t("unit.hoursMinutesShort").arg(hours).arg(pad(minutes))
      if (minutes > 0) return strings.t("unit.minutesShort").arg(minutes)
      return strings.t("unit.secondsShort").arg(secs)
    }
    return strings.t("unit.clock").arg(pad(hours)).arg(pad(minutes)).arg(pad(secs))
  }

  function formatMinutes(value) {
    return strings.t("unit.minutesShort").arg(Math.max(0, Math.floor(Number(value) || 0)))
  }

  function countdownDeadline(status) {
    var next = status && status.next ? status.next : null
    if (!next) return 0
    var at = Date.parse(String(next.at || ""))
    if (!isNaN(at)) return at
    // The CLI fallback reports a snapshot rather than next.at. Converting it
    // once for the display countdown does not add any boundary rule.
    return Date.now() + Math.max(0, Number(next.remaining_seconds) || 0) * 1000
  }

  function refresh() {
    client.refreshStatus()
  }

  function beginOp(kind, rowKey, tempKey) {
    actionError = ""
    var opId = client.newOpId()
    var entry = { "kind": String(kind || ""), "rowKey": String(rowKey || "") }
    if (tempKey) entry.tempKey = String(tempKey)
    pendingOps[opId] = entry
    return opId
  }

  function clearPending(opId) {
    var pending = pendingOps[opId]
    if (!pending) return
    delete pendingOps[opId]
    var content = panel.loadedContent
    if (content && pending.rowKey) content.setRowPending(pending.rowKey, false)
    if (content && pending.tempKey) content.removeItem(pending.tempKey)
  }

  function addItem(title) {
    var text = String(title || "").trim()
    if (text === "") return
    var content = panel.loadedContent
    var opId = beginOp("add", "")
    var tempKey = "pending:" + opId
    pendingOps[opId].rowKey = tempKey
    pendingOps[opId].tempKey = tempKey
    if (content) content.addPendingItem(text, quickAddBlock, tempKey)
    client.addItem(opId, text, quickAddBlock)
  }

  function presetQuickAdd(block) {
    quickAddBlock = String(block || "")
    var content = panel.loadedContent
    if (content && content.quickField) {
      content.quickField.forceActiveFocus()
      content.quickField.cursorPosition = content.quickField.text.length
    }
  }

  function clearQuickAddPreset() {
    quickAddBlock = ""
  }

  function prayerName(id, fallback) {
    if (String(id || "") === "night") return strings.t("block.night")
    var key = "prayer." + String(id || "")
    var translated = strings.t(key)
    return translated === key ? String(fallback || id || "") : translated
  }

  function isPrayerMarked(id) {
    var marks = planner && planner.prayer_marks ? planner.prayer_marks : []
    return marks.indexOf(String(id || "")) >= 0
  }

  function togglePrayer(prayer) {
    var id = String(prayer.id)
    var marked = !isPrayerMarked(id)
    var opId = beginOp("prayer", "header:" + id)
    client.markPrayer(opId, id, marked)
  }

  function boundary(id) {
    var rows = report ? (report.boundaries || []) : []
    for (var i = 0; i < rows.length; i++)
      if (String(rows[i].id || "") === id) return rows[i]
    return null
  }

  function toggleItem(row) {
    var wasDone = row.state === "done"
    var opId = beginOp("done", "item:" + String(row.id))
    client.setDone(opId, String(row.id), !wasDone)
  }

  function toggleTimer(row) {
    var running = row.timerRunning === true
    var opId = beginOp(running ? "stopTimer" : "startTimer", "item:" + String(row.id))
    if (running) client.stopTimer(opId, String(row.id))
    else client.startTimer(opId, String(row.id))
  }

  function adjustCount(row, delta) {
    var amount = delta < 0 ? -1 : 1
    var opId = beginOp("logCount", "item:" + String(row.id))
    client.logCount(opId, String(row.id), amount)
  }

  function correctMinutes(row, amount) {
    var opId = beginOp("logMinutes", "item:" + String(row.id))
    client.logMinutes(opId, String(row.id), amount)
  }

  function editItem(row, title) {
    var text = String(title || "").trim()
    if (text === "" || text === String(row.title || "")) return
    var opId = beginOp("edit", "item:" + String(row.id))
    client.editItem(opId, String(row.id), text)
  }

  function removeItem(row) {
    var opId = beginOp("remove", "item:" + String(row.id))
    client.removeItem(opId, String(row.id))
  }

  function syncLocation(force) {
    // Keep at most one online lookup in flight. The Rust command validates
    // and atomically stores the response, so status only runs after it exits.
    if (locationProcess.running) {
      locationSyncPendingStatus = true
      return
    }
    locationSyncPendingStatus = true
    locationProcess.command = force === true
      ? ["salah", "location", "sync", "--json"]
      : ["salah", "location", "sync", "--if-needed", "--json"]
    locationProcess.running = true
  }

  function open() {
    panel.open = true
    refresh()
  }

  function close() {
    var content = panel.loadedContent
    if (content && content.collapseExpanded) content.collapseExpanded()
    panel.open = false
  }
  function toggle() { opened ? close() : open() }
  function closeForPopoutSwitch() {
    popoutSwitchClosing = true
    close()
    Qt.callLater(function() { root.popoutSwitchClosing = false })
  }

  function acceptReport(status) {
    try {
      if (!status || !status.next || !(status.prayers instanceof Array))
        throw new Error("missing next/prayers fields")
      report = status
      timerSnapshotMs = Date.now()
      countdownAtMs = countdownDeadline(status)
      countdownNowMs = Date.now()
      var content = panel.loadedContent
      if (content) content.syncTimeline(status)
      errorMessage = ""
    } catch (error) {
      errorMessage = String(error)
    }
    firstLoad = false
  }

  function reportFailure(raw) {
    var detail = String(raw || "").trim().replace(/[\r\n]+/g, " ")
    errorMessage = detail === "" ? strings.t("panel.commandFailed") : detail.slice(0, 240)
    firstLoad = false
  }

  function applyChanged(changed) {
    if (!report || !report.planner || !changed) return
    var plan = report.planner
    var rows = (plan.items || []).slice()
    var items = changed.items || []
    for (var i = 0; i < items.length; i++) {
      var source = items[i]
      var found = -1
      for (var j = 0; j < rows.length; j++)
        if (String(rows[j].id || "") === String(source.id || "")) { found = j; break }
      if (found >= 0) rows[found] = source
      else rows.push(source)
    }
    var deleted = changed.deleted || []
    for (var d = 0; d < deleted.length; d++) {
      var id = String(deleted[d])
      rows = rows.filter(function(item) { return String(item.id || "") !== id })
    }
    var marks = (plan.prayer_marks || []).slice()
    var changedMarks = changed.marks || []
    for (var m = 0; m < changedMarks.length; m++) {
      var mark = changedMarks[m]
      var prayer = String(mark.prayer || "")
      var at = marks.indexOf(prayer)
      if (mark.deleted_at) {
        if (at >= 0) marks.splice(at, 1)
      } else if (at < 0) {
        marks.push(prayer)
      }
    }
    plan.items = rows
    plan.prayer_marks = marks
    plan.timer = null
    report = Object.assign({}, report, { "planner": plan })
    timerSnapshotMs = Date.now()
    var content = panel.loadedContent
    if (content) content.syncTimeline(report)
  }

  function handleMutationSuccess(opId, kind, result) {
    if (result && result.fallback) {
      clearPending(opId)
      if (kind === "add" && result.raw) {
        var raw = null
        try { raw = JSON.parse(String(result.raw || "").trim()) } catch (error) { raw = null }
        var fallbackContent = panel.loadedContent
        if (fallbackContent && raw) fallbackContent.showNextOccurrence(raw)
      }
      client.refreshStatus()
      return
    }
    if (result && result.changed) applyChanged(result.changed)
    if (kind === "add" && result && result.changed && result.changed.items
        && result.changed.items.length > 0) {
      var target = panel.loadedContent
      if (target) target.showNextOccurrence(result.changed.items[0])
    }
    clearPending(opId)
  }

  function handleMutationFailure(opId, kind, message) {
    clearPending(opId)
    var detail = String(message || "").trim().replace(/[\r\n]+/g, " ")
    actionError = detail === "" ? strings.t("panel.commandFailed") : detail.slice(0, 240)
    client.refreshStatus()
  }

  function parseLocationConfig(raw) {
    try {
      var config = JSON.parse(String(raw || "").trim())
      if (!config || typeof config !== "object") return
      root.autoLocation = config.auto_location !== false
      root.lastLocationSync = String(config.last_location_sync || "")
    } catch (error) {
      // A successful command has already validated and saved its response.
      // Status remains authoritative, so malformed optional metadata is safe
      // to ignore rather than turning a good location update into a failure.
    }
  }

  Component.onCompleted: syncLocation(false)

  Timer {
    interval: 1000
    running: true
    repeat: true
    onTriggered: root.countdownNowMs = Date.now()
  }

  Timer {
    interval: 60000
    running: root.runningTimer !== null
    repeat: true
    triggeredOnStart: false
    onTriggered: root.minuteTick++
  }

  Timer {
    // Only the CLI fallback needs polling; a connected socket pushes changes.
    interval: 30000
    running: !client.socketActive
    repeat: true
    triggeredOnStart: false
    onTriggered: root.refresh()
  }

  Timer {
    // IP-derived coordinates change rarely. Six hours limits network use
    // while still following travel during a normal day.
    interval: 6 * 60 * 60 * 1000
    running: true
    repeat: true
    triggeredOnStart: false
    onTriggered: root.syncLocation(false)
  }

  SalahClient {
    id: client
    onStatusLoaded: function(status) { root.acceptReport(status) }
    onStatusFailed: function(message) { root.reportFailure(message) }
    onMutationSucceeded: function(opId, kind, result) { root.handleMutationSuccess(opId, kind, result) }
    onMutationFailed: function(opId, kind, message) { root.handleMutationFailure(opId, kind, message) }
    onTransportChanged: function(active) { if (!active) root.refresh() }
  }

  Process {
    id: locationProcess
    command: ["salah", "location", "sync", "--if-needed", "--json"]
    stdout: StdioCollector {
      id: locationStdout
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: locationStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      if (exitCode === 0) {
        root.parseLocationConfig(locationStdout.text)
        root.locationWarning = ""
      } else {
        var detail = String(locationStderr.text || "").trim().replace(/[\r\n]+/g, " ")
        root.locationWarning = detail === ""
          ? strings.t("panel.locationSyncFailed")
          : detail.slice(0, 240)
      }

      if (!officialProcess.running) officialProcess.running = true
    }
  }

  Process {
    id: officialProcess
    command: ["salah", "official", "sync", "--json"]
    stderr: StdioCollector { id: officialStderr; waitForEnd: true }
    onExited: function(exitCode) {
      if (exitCode === 0) root.officialWarning = ""
      else {
        var detail = String(officialStderr.text || "").trim().replace(/[\r\n]+/g, " ")
        root.officialWarning = detail === ""
          ? strings.t("panel.officialSyncFailed")
          : detail.slice(0, 240)
      }
      root.locationSyncPendingStatus = false
      root.refresh()
    }
  }

  implicitWidth: root.vertical ? button.implicitWidth
    : Style.space(root.runningTimer ? 250 : 180)
  implicitHeight: button.implicitHeight

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.vertical ? strings.t("bar.symbol") : root.displayText
    labelVisible: true
    fixedWidth: root.vertical ? -1 : root.width
    fontFamily: root.runningTimer ? "monospace" : (root.bar ? root.bar.fontFamily : Style.font.family)
    horizontalMargin: 8.75
    verticalPadding: 8.75
    tooltipText: root.localizedNextName !== ""
      ? strings.t("bar.tooltip").arg(root.localizedNextName).arg(root.nextTime).arg(root.remainingLong)
      : strings.t("bar.unavailable")
    pressFeedbackButtons: Qt.NoButton
    onPressed: function(mouseButton) {
      if (mouseButton === Qt.MiddleButton) root.syncLocation(true)
      else if (mouseButton === Qt.LeftButton) root.toggle()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: false
    focusTarget: loadedContent
    contentWidth: fittedContentWidth(Style.space(520))
    contentHeight: fittedContentHeight(loadedContentHeight)

    content: PanelKeyCatcher {
      id: keyCatcher
      property alias quickField: quickAdd.quickField
      property alias timelineModel: timelineModel
      property alias timelineList: timelineList
      property string dragKey: ""
      property var dragSnapshot: []
      property real dragPointerY: 0
      property string dragTitle: ""
      property string editingKey: ""
      property string expandedItemId: ""
      property string occurrenceHint: ""
      anchors.fill: parent
      implicitHeight: Math.min(dayColumn.implicitHeight,
        Math.max(Style.space(240), panel.availableCardHeight - panel.padding * 2))
      blocked: quickField.activeFocus || editingKey !== ""
      function collapseExpanded() {
        expandedItemId = ""
        editingKey = ""
      }

      function toggleExpanded(id) {
        var next = expandedItemId === id ? "" : id
        expandedItemId = next
        editingKey = ""
        if (next !== "") Qt.callLater(function() {
          var index = indexOfKey("item:" + next)
          var delegate = index >= 0 ? timelineList.itemAtIndex(index) : null
          if (delegate && delegate.focusTimeField) delegate.focusTimeField()
        })
      }

      onCloseRequested: {
        if (dragKey !== "") cancelDrag()
        else if (expandedItemId !== "") collapseExpanded()
        else root.close()
      }
      onTabRequested: function(direction) {
        if (root.bar && typeof root.bar.switchPanelFrom === "function")
          root.bar.switchPanelFrom(root, direction)
      }
      onActivateRequested: {
        var row = timelineModel.get(timelineList.currentIndex)
        if (row && row.rowType === "item" && !row.pending) toggleExpanded(row.itemId)
        else root.refresh()
      }

      function baseRow(type, key, block) {
        return {
          "rowType": type, "rowKey": key, "block": block || "", "itemId": "",
          "title": "", "time": "", "itemState": "", "measure": "check", "target": 0,
          "minutes": 0, "count": 0, "repeat": "none", "timerRunning": false,
          "timerStartedAt": "", "timerMinutes": 0, "detailText": "",
          "scheduleText": "", "nextOccurrence": "", "carried": false,
          "current": false, "marked": false, "passed": false, "pending": false
        }
      }

      function blockName(block, fallback) {
        return root.prayerName(block, fallback || block)
      }

      function desiredRows(status) {
        var result = []
        var plan = status && status.planner ? status.planner : null
        var items = plan && plan.items ? plan.items : []
        var prayers = status && status.prayers ? status.prayers : []
        var blocks = ["night", "fajr", "dhuhr", "asr", "maghrib", "isha"]
        var currentBlock = plan ? String(plan.block || "") : ""
        if (currentBlock === "" && status && status.current_block && status.current_block.start)
          currentBlock = String(status.current_block.start.id || "")
        for (var b = 0; b < blocks.length; b++) {
          var block = blocks[b]
          var prayer = null
          for (var p = 0; p < prayers.length; p++)
            if (String(prayers[p].id || "") === block) prayer = prayers[p]
          var header = baseRow("header", "header:" + block, block)
          header.title = blockName(block, prayer ? prayer.name : block)
          header.time = block === "night"
            ? String((plan && (plan.night_start || plan.sleep_time))
                || status.night_start || status.sleep_time || "")
            : String(prayer ? prayer.time || "" : "")
          header.current = currentBlock === block
          header.marked = block !== "night" && root.isPrayerMarked(block)
          header.passed = prayer ? prayer.passed === true : false
          result.push(header)

          // The core returns rows ordered by block then position; grouping a
          // block's rows is row assembly, not a re-sort.
          for (var i = 0; i < items.length; i++) {
            var source = items[i]
            if (String(source.block || "") !== block) continue
            var timer = source.timer || {}
            var item = baseRow("item", "item:" + String(source.id), block)
            item.itemId = String(source.id || "")
            item.title = String(source.title || "")
            item.itemState = String(source.state || "open")
            item.measure = String(source.measure || "check")
            item.target = Number(source.target) || 0
            item.minutes = Number(source.minutes) || 0
            item.count = Number(source.count) || 0
            item.repeat = String(source.repeat || "none")
            item.timerRunning = timer.running === true
            item.timerStartedAt = String(timer.started_at || "")
            item.timerMinutes = Number(timer.minutes_including_running) || 0
            item.detailText = String(source.detail_text || "")
            item.scheduleText = String(source.schedule_text || "")
            item.nextOccurrence = source.next_occurrence ? String(source.next_occurrence) : ""
            item.carried = source.carried === true
            result.push(item)
          }
          var add = baseRow("add", "add:" + block, block)
          add.title = strings.t("panel.addTo").arg(header.title)
          result.push(add)
          if (block === "fajr" && root.boundary("sunrise")) {
            var sunrise = baseRow("marker", "marker:sunrise", block)
            sunrise.title = strings.t("boundary.sunrise")
            sunrise.time = String(root.boundary("sunrise").time || "")
            result.push(sunrise)
          }
        }
        var sleep = baseRow("marker", "marker:sleep", "isha")
        sleep.title = strings.t("boundary.sleep")
        var sleepBoundary = root.boundary("sleep")
        sleep.time = String(sleepBoundary ? sleepBoundary.time || "" : status.sleep_time || "")
        result.push(sleep)
        return result
      }

      function indexOfKey(key) {
        for (var i = 0; i < timelineModel.count; i++)
          if (timelineModel.get(i).rowKey === key) return i
        return -1
      }

      function syncTimeline(status) {
        // A server push during a drag is applied once the drag ends.
        if (dragKey !== "") return
        var desired = desiredRows(status)
        var desiredKeys = ({})
        for (var k = 0; k < desired.length; k++) desiredKeys[desired[k].rowKey] = true
        for (var extra = timelineModel.count - 1; extra >= 0; extra--) {
          var key = timelineModel.get(extra).rowKey
          if (!desiredKeys[key] && String(key).indexOf("pending:") !== 0)
            timelineModel.remove(extra)
        }
        for (var i = 0; i < desired.length; i++) {
          var found = indexOfKey(desired[i].rowKey)
          if (found < 0) timelineModel.insert(i, desired[i])
          else {
            if (found !== i) timelineModel.move(found, i, 1)
            var row = desired[i]
            for (var role in row)
              if (timelineModel.get(i)[role] !== row[role]) timelineModel.setProperty(i, role, row[role])
          }
        }
        reapplyPending()
        if (expandedItemId !== "" && indexOfKey("item:" + expandedItemId) < 0)
          collapseExpanded()
      }

      function setRowPending(key, value) {
        var index = indexOfKey(key)
        if (index >= 0) timelineModel.setProperty(index, "pending", value)
      }

      function reapplyPending() {
        for (var opId in root.pendingOps) {
          var pending = root.pendingOps[opId]
          if (!pending) continue
          if (pending.rowKey) setRowPending(pending.rowKey, true)
          if (pending.tempKey) {
            var temp = indexOfKey(pending.tempKey)
            if (temp >= 0) {
              var block = timelineModel.get(temp).block
              var addIndex = indexOfKey("add:" + block)
              if (addIndex >= 0) {
                var target = temp < addIndex ? addIndex - 1 : addIndex
                if (temp !== target) timelineModel.move(temp, target, 1)
              }
            }
          }
        }
      }

      function removeItem(key) {
        var index = indexOfKey(key)
        if (index < 0) index = indexOfKey("item:" + key)
        if (index >= 0) timelineModel.remove(index)
      }

      function weekdayName(index) {
        var keys = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"]
        return strings.t("weekday." + keys[index])
      }

      function showNextOccurrence(item) {
        var next = item ? String(item.next_occurrence || "") : ""
        var today = root.planner ? String(root.planner.day || "") : ""
        if (next === "" || today === "" || next <= today) return
        var parts = next.split("-")
        if (parts.length !== 3) return
        var date = new Date(Number(parts[0]), Number(parts[1]) - 1, Number(parts[2]))
        occurrenceHint = strings.t("panel.nextOccurrence")
          .arg(String(item.title || "")).arg(weekdayName(date.getDay()))
        occurrenceHintTimer.restart()
      }

      function addPendingItem(title, requestedBlock, key) {
        var block = String(requestedBlock || "")
        if (block === "" && root.planner) {
          var planning = root.planner.planning_block
          block = String(planning && planning.start || root.planner.block || "")
        }
        var addIndex = indexOfKey("add:" + block)
        if (addIndex < 0) return
        var pending = baseRow("item", key || ("pending:" + Date.now()), block)
        pending.title = title
        pending.itemId = pending.rowKey
        pending.pending = true
        timelineModel.insert(addIndex, pending)
      }

      function timelineRows() {
        var rows = []
        for (var i = 0; i < timelineModel.count; i++) rows.push(timelineModel.get(i))
        return rows
      }

      function beginDrag(key, title, pointerY) {
        if (dragKey !== "" || key.indexOf("item:") !== 0) return
        dragSnapshot = []
        for (var i = 0; i < timelineModel.count; i++) dragSnapshot.push(timelineModel.get(i).rowKey)
        dragKey = key
        dragTitle = title
        dragPointerY = pointerY
        timelineList.forceActiveFocus()
      }

      function moveDrag(pointerY) {
        if (dragKey === "") return
        var height = timelineList.contentHeight
        var outside = pointerY < 0 ? "above" : (pointerY >= height ? "below" : "")
        dragPointerY = Math.max(0, Math.min(height, pointerY))
        var from = indexOfKey(dragKey)
        if (from < 0) return
        var hit = -1
        var afterHit = false
        if (outside === "") {
          hit = timelineList.indexAt(Style.space(10), dragPointerY)
          if (hit < 0) outside = dragPointerY <= 0 ? "above" : "below"
        }
        if (hit >= 0) {
          var delegate = timelineList.itemAtIndex(hit)
          afterHit = delegate ? dragPointerY > delegate.y + delegate.height / 2 : false
        }
        var target = DragLogic.targetIndex(timelineRows(), dragKey, hit, afterHit, outside)
        if (target >= 0 && target !== from && target < timelineModel.count)
          timelineModel.move(from, target, 1)
      }

      function restoreDragOrder() {
        for (var i = 0; i < dragSnapshot.length; i++) {
          var found = indexOfKey(dragSnapshot[i])
          if (found >= 0 && found !== i) timelineModel.move(found, i, 1)
        }
      }

      // Rebuild the visual order from the latest server rows. Every drop,
      // cancel, and failed move ends here so the list can never stay in a
      // position the server would not accept.
      function restoreServerOrder() {
        if (root.report) syncTimeline(root.report)
        else restoreDragOrder()
      }

      function cancelDrag() {
        if (dragKey === "") return
        dragKey = ""
        dragSnapshot = []
        restoreServerOrder()
      }

      function finishDrag(inside) {
        if (dragKey === "") return
        if (!inside) {
          cancelDrag()
          return
        }
        var key = dragKey
        var from = indexOfKey(key)
        var id = from >= 0 ? String(timelineModel.get(from).itemId || "") : ""
        var target = DragLogic.destination(timelineRows(), key)
        dragKey = ""
        dragSnapshot = []
        // Invalid final positions cancel: never send a move and never leave
        // the row where the drag happened to stop.
        if (id === "" || !target || target.valid !== true || target.block === "") {
          restoreServerOrder()
          return
        }
        // Show the authoritative server order while the move is in flight;
        // the response (or changed notification) rebuilds it again.
        restoreServerOrder()
        var opId = root.beginOp("move", "item:" + id)
        setRowPending("item:" + id, true)
        client.moveItem(opId, id, target.block, target.index,
          String(target.after_id || ""), String(target.before_id || ""))
      }

      ListModel {
        id: timelineModel
        dynamicRoles: true
      }

      Timer {
        interval: 16
        repeat: true
        running: keyCatcher.dragKey !== ""
        onTriggered: {
          var edge = Style.space(40)
          var viewportY = flatTimeline.y + keyCatcher.dragPointerY - dayScroll.contentY
          var delta = 0
          if (viewportY < edge) delta = -Math.min(Style.space(8), edge - viewportY)
          else if (viewportY > dayScroll.height - edge)
            delta = Math.min(Style.space(8), viewportY - (dayScroll.height - edge))
          var nextY = Math.max(0, Math.min(dayScroll.contentHeight - dayScroll.height,
            dayScroll.contentY + delta))
          var applied = nextY - dayScroll.contentY
          if (applied !== 0) {
            dayScroll.contentY = nextY
            keyCatcher.moveDrag(keyCatcher.dragPointerY + applied)
          }
        }
      }

      Timer {
        id: occurrenceHintTimer
        interval: 5000
        onTriggered: keyCatcher.occurrenceHint = ""
      }

      Component.onCompleted: if (root.report) syncTimeline(root.report)

      Flickable {
        id: dayScroll
        anchors.fill: parent
        contentWidth: width
        contentHeight: dayColumn.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height

        Column {
          id: dayColumn
          width: dayScroll.width
          spacing: Style.space(10)

          Item {
            width: parent.width
            implicitHeight: heroColumn.implicitHeight

            Column {
              id: heroColumn
              anchors.left: parent.left
              anchors.right: heroTime.left
              anchors.rightMargin: Style.space(12)
              spacing: Style.space(2)

              PanelSectionHeader {
                width: parent.width
                text: strings.t("panel.next")
                foreground: root.foreground
                fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
              }

              Text {
                width: parent.width
                textFormat: Text.PlainText
                text: root.prayerName(root.report && root.report.next ? root.report.next.id : "", root.nextName)
                  || strings.t("panel.noTimeSymbol")
                color: root.foreground
                font.family: root.bar ? root.bar.fontFamily : Style.font.family
                font.pixelSize: Style.font.display
                font.bold: true
                elide: Text.ElideRight
              }

              Text {
                width: parent.width
                textFormat: Text.PlainText
                text: strings.t("panel.beginsIn").arg(root.remainingLong)
                color: root.mutedForeground
                font.family: root.bar ? root.bar.fontFamily : Style.font.family
                font.pixelSize: Style.font.bodySmall
              }
            }

            Text {
              id: heroTime
              anchors.right: parent.right
              anchors.verticalCenter: heroColumn.verticalCenter
              textFormat: Text.PlainText
              text: root.nextTime || strings.t("panel.noTimeSymbol")
              color: root.foreground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.displayLarge
              font.bold: true
            }
          }

          QuickAdd {
            id: quickAdd
            strings: root.tr
            host: root
            fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
            foreground: root.foreground
            mutedForeground: root.mutedForeground
            occurrenceHint: keyCatcher.occurrenceHint
            actionError: root.actionError
          }

          PanelSeparator {
            visible: root.report !== null
            foreground: root.foreground
          }

          Item {
            id: flatTimeline
            visible: root.report !== null
            width: parent.width
            height: timelineList.contentHeight

            ListView {
              id: timelineList
              anchors.fill: parent
              interactive: false
              model: timelineModel
              clip: false
              focus: keyCatcher.dragKey !== ""

              move: Transition {
                NumberAnimation {
                  properties: "y"
                  duration: Style.duration(120)
                  easing.type: Easing.OutCubic
                }
              }
              displaced: Transition {
                NumberAnimation {
                  properties: "y"
                  duration: Style.duration(120)
                  easing.type: Easing.OutCubic
                }
              }

              Keys.onEscapePressed: function(event) {
                if (keyCatcher.dragKey !== "") {
                  keyCatcher.cancelDrag()
                  event.accepted = true
                }
              }

              delegate: TimelineRow {
                host: root
                controller: keyCatcher
                timeline: timelineList
                strings: root.tr
                fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
                foreground: root.foreground
                mutedForeground: root.mutedForeground
              }
            }

            Rectangle {
              visible: keyCatcher.dragKey !== ""
              x: Style.space(28)
              y: Math.max(0, Math.min(flatTimeline.height - height,
                keyCatcher.dragPointerY - height / 2))
              width: flatTimeline.width - Style.space(28)
              height: Style.space(38)
              z: 20
              radius: Style.cornerRadius
              color: Commons.Color.background
              border.width: Style.spacing.hairline
              border.color: Style.selectedStateColor(root.foreground, Commons.Color.accent)
              scale: 1.02

              Text {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.leftMargin: Style.space(12)
                anchors.rightMargin: Style.space(12)
                anchors.verticalCenter: parent.verticalCenter
                textFormat: Text.PlainText
                text: keyCatcher.dragTitle
                color: root.foreground
                font.family: root.bar ? root.bar.fontFamily : Style.font.family
                font.pixelSize: Style.font.bodySmall
                elide: Text.ElideRight
              }
            }
          }

          Text {
            visible: root.report && root.report.planner_error
            width: parent.width
            textFormat: Text.PlainText
            text: strings.t("panel.plannerUnavailable")
            color: root.mutedForeground
            font.family: root.bar ? root.bar.fontFamily : Style.font.family
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
          }

          Column {
            visible: root.report === null
            width: parent.width
            spacing: Style.space(5)

            Text {
              width: parent.width
              textFormat: Text.PlainText
              text: strings.t("panel.noData")
              color: root.foreground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.body
              font.bold: true
              wrapMode: Text.WordWrap
            }

            Text {
              visible: root.errorMessage !== ""
              width: parent.width
              textFormat: Text.PlainText
              text: root.errorMessage
              color: root.mutedForeground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.caption
              wrapMode: Text.WrapAnywhere
              maximumLineCount: 3
              elide: Text.ElideRight
            }

            Text {
              width: parent.width
              textFormat: Text.PlainText
              text: strings.t("panel.retryHint")
              color: root.mutedForeground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.WordWrap
            }
          }

          PanelSeparator {
            foreground: root.foreground
          }

          Text {
            visible: root.locationWarning !== ""
              || (root.officialWarning !== "" && root.report && root.report.source !== "official")
            width: parent.width
            textFormat: Text.PlainText
            text: root.locationWarning !== ""
              ? strings.t("panel.usingSavedLocation").arg(root.locationWarning)
              : strings.t("panel.usingCalculatedTimes").arg(root.officialWarning)
            color: root.mutedForeground
            font.family: root.bar ? root.bar.fontFamily : Style.font.family
            font.pixelSize: Style.font.caption
            wrapMode: Text.WrapAnywhere
            maximumLineCount: 2
            elide: Text.ElideRight
          }

          Item {
            visible: root.report !== null
            width: parent.width
            implicitHeight: Math.max(locationLine.implicitHeight, refreshButton.implicitHeight)

            Text {
              id: locationLine
              anchors.left: parent.left
              anchors.right: refreshButton.left
              anchors.rightMargin: Style.space(6)
              anchors.verticalCenter: parent.verticalCenter
              textFormat: Text.PlainText
              text: root.report
                ? strings.t("panel.locationSource")
                    .arg(String(root.report.location || ""))
                    .arg(String(root.report.source_name || root.report.method_name || root.report.method || ""))
                : ""
              color: root.mutedForeground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.caption
              elide: Text.ElideRight

              MouseArea {
                id: locationHover
                anchors.fill: parent
                hoverEnabled: root.autoLocation
                acceptedButtons: Qt.NoButton
              }

              PanelToolTip {
                visible: root.autoLocation && locationHover.containsMouse
                text: strings.t("panel.networkLocationNotice")
                fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
              }
            }

            Button {
              id: refreshButton
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              width: Style.space(32)
              height: Style.space(28)
              iconText: strings.t("panel.refreshSymbol")
              tooltipText: strings.t("panel.refreshNow")
              foreground: root.foreground
              fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
              horizontalPadding: Style.space(5)
              verticalPadding: Style.space(2)
              onClicked: root.syncLocation(true)
            }
          }
        }
      }
    }
  }
}
