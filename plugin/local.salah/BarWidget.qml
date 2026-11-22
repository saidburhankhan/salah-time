import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons
import qs.Ui

BarWidget {
  id: root
  moduleName: "local.salah"

  Commons.Translations {
    id: strings
    directory: Qt.resolvedUrl("i18n")
  }

  property var report: null
  property int remainingSeconds: report && report.next
    ? Math.max(0, Number(report.next.remaining_seconds) || 0)
    : 0
  property string errorMessage: ""
  property string locationWarning: ""
  property string officialWarning: ""
  property bool locationSyncPendingStatus: false
  property bool autoLocation: true
  property string lastLocationSync: ""
  property bool firstLoad: true
  property bool popoutSwitchClosing: false
  property string actionError: ""
  property var pendingActions: []
  property string quickAddBlock: ""
  property string lastStatusText: ""
  property int optimisticOpenDelta: 0
  property int minuteTick: 0
  property var activeAction: null
  property var optimisticTimer: undefined
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
  readonly property var runningTimer: optimisticTimer !== undefined
    ? optimisticTimer : (planner && planner.timer ? planner.timer : null)
  readonly property int runningMinutes: {
    minuteTick
    if (!runningTimer) return 0
    var base = Math.max(0, Number(runningTimer.minutes) || 0)
    var started = Date.parse(String(runningTimer.started_at || ""))
    if (isNaN(started)) return base
    var thenMinutes = Math.max(0, Math.floor((timerSnapshotMs - started) / 60000))
    var nowMinutes = Math.max(0, Math.floor((Date.now() - started) / 60000))
    return base + Math.max(0, nowMinutes - thenMinutes)
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

  function refresh() {
    // Preserve optimistic UI while commands are queued; the last command
    // schedules the one authoritative refresh for the whole burst.
    if (actionProcess.running || pendingActions.length > 0) return
    if (!statusProcess.running) statusProcess.running = true
  }

  // Planner actions run one at a time, in order, as fixed argument lists.
  // Titles follow `--` so text starting with "-" is never read as a flag.
  function runAction(args, kind) {
    if (!actionProcess.running && pendingActions.length === 0) actionError = ""
    pendingActions = pendingActions.concat([{
      "command": ["salah"].concat(args), "kind": String(kind || "")
    }])
    runNextAction()
  }

  function runNextAction() {
    if (actionProcess.running || pendingActions.length === 0) return
    activeAction = pendingActions[0]
    actionProcess.command = activeAction.command
    pendingActions = pendingActions.slice(1)
    actionProcess.running = true
  }

  function addItem(title) {
    var text = String(title || "").trim()
    if (text === "") return
    var args = ["item", "add", "--json", "--quick"]
    if (quickAddBlock !== "") args = args.concat(["--block", quickAddBlock])
    var content = panel.loadedContent
    if (content) content.addPendingItem(text, quickAddBlock)
    optimisticOpenDelta++
    runAction(args.concat(["--", text]), "add")
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
    var content = panel.loadedContent
    var marked = content ? content.togglePrayerOptimistic(String(prayer.id)) : !isPrayerMarked(prayer.id)
    runAction(["prayer", marked ? "mark" : "unmark", "--json", String(prayer.id)])
  }

  function itemsForBlock(block, source) {
    var rows = source || (planner ? (planner.items || []) : [])
    var result = []
    for (var i = 0; i < rows.length; i++)
      if (String(rows[i].block || "") === String(block || "")) result.push(rows[i])
    result.sort(function(a, b) { return (Number(a.position) || 0) - (Number(b.position) || 0) })
    return result
  }

  function boundary(id) {
    var rows = report ? (report.boundaries || []) : []
    for (var i = 0; i < rows.length; i++)
      if (String(rows[i].id || "") === id) return rows[i]
    return null
  }

  function toggleItem(row) {
    var wasDone = row.state === "done"
    var content = panel.loadedContent
    if (content) content.patchItem(String(row.id), { "state": wasDone ? "open" : "done" })
    optimisticOpenDelta += wasDone ? 1 : -1
    runAction(["item", wasDone ? "undo" : "done", "--json", String(row.id)])
  }

  function toggleTimer(row) {
    var running = String(row.timer_started_at || "") !== ""
    var content = panel.loadedContent
    if (content) content.setRunningTimer(running ? "" : String(row.id))
    optimisticTimer = running ? null : {
      "item_id": String(row.id), "title": String(row.title || ""),
      "started_at": new Date().toISOString(), "minutes": Number(row.minutes) || 0
    }
    timerSnapshotMs = Date.now()
    if (running) runAction(["item", "stop", "--json", String(row.id)])
    else runAction(["item", "start", "--json", String(row.id)])
  }

  function adjustCount(row, delta) {
    var amount = delta < 0 ? -1 : 1
    var current = Math.max(0, Number(row.count) || 0)
    var next = Math.max(0, current + amount)
    var content = panel.loadedContent
    if (content) content.patchItem(String(row.id), { "count": next })
    runAction(["item", "log", "--json", String(row.id), String(amount)])
  }

  function correctMinutes(row, amount) {
    var current = Math.max(0, Number(row.minutes) || 0)
    var next = Math.max(0, current + amount)
    var content = panel.loadedContent
    if (content) content.patchItem(String(row.id), { "minutes": next })
    if (runningTimer && String(runningTimer.item_id || "") === String(row.id)) {
      optimisticTimer = {
        "item_id": String(row.id), "title": String(runningTimer.title || row.title || ""),
        "started_at": String(runningTimer.started_at || row.timer_started_at || ""), "minutes": next
      }
    }
    runAction(["item", "log", "--json", "--minutes", String(row.id), String(amount)])
  }

  function editItem(row, title) {
    var text = String(title || "").trim()
    if (text === "" || text === String(row.title || "")) return
    var content = panel.loadedContent
    if (content) content.patchItem(String(row.id), { "title": text })
    runAction(["item", "edit", "--json", String(row.id), "--title=" + text])
  }

  function removeItem(row) {
    var content = panel.loadedContent
    if (content) content.removeItem(String(row.id))
    if (row.state !== "done") optimisticOpenDelta--
    runAction(["item", "remove", "--json", String(row.id)])
  }

  function itemDetail(row) {
    var parts = []
    if (row.measure === "minutes")
      parts.push(strings.t("panel.timeProgress").arg(row.minutes).arg(row.target))
    if (row.repeat === "daily") parts.push(strings.t("repeat.daily"))
    else if (row.repeat === "weekly") parts.push(strings.t("repeat.weekly"))
    return parts.join(strings.t("panel.detailSeparator"))
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

  function parseReport(raw) {
    try {
      var text = String(raw || "").trim()
      if (text === lastStatusText) {
        firstLoad = false
        return
      }
      var parsed = JSON.parse(text)
      if (!parsed || !parsed.next || !(parsed.prayers instanceof Array))
        throw new Error("missing next/prayers fields")
      if (actionProcess.running || pendingActions.length > 0) {
        firstLoad = false
        return
      }
      report = parsed
      optimisticTimer = undefined
      timerSnapshotMs = Date.now()
      lastStatusText = text
      optimisticOpenDelta = 0
      var content = panel.loadedContent
      if (content) content.syncTimeline(parsed)
      remainingSeconds = Math.max(0, Number(parsed.next.remaining_seconds) || 0)
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
    onTriggered: {
      if (root.remainingSeconds > 0) {
        root.remainingSeconds--
        // Refresh exactly once at the boundary. The periodic timer handles a
        // transient failure instead of spawning one process every second.
        if (root.remainingSeconds === 0) root.refresh()
      }
    }
  }

  Timer {
    interval: 60000
    running: root.runningTimer !== null
    repeat: true
    triggeredOnStart: false
    onTriggered: root.minuteTick++
  }

  Timer {
    interval: 30000
    running: true
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

  Process {
    id: actionProcess
    stdout: StdioCollector { id: actionStdout; waitForEnd: true }
    stderr: StdioCollector {
      id: actionStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      if (exitCode !== 0) {
        var detail = String(actionStderr.text || "").trim().replace(/[\r\n]+/g, " ")
        root.actionError = detail === "" ? strings.t("panel.commandFailed") : detail.slice(0, 240)
        // The next status result must reconcile the optimistic edit even when
        // the backend JSON is byte-for-byte equal to the previous result.
        root.lastStatusText = ""
      } else if (root.activeAction && root.activeAction.kind === "add") {
        var content = panel.loadedContent
        if (content) content.showNextOccurrence(actionStdout.text)
      }
      root.activeAction = null
      root.runNextAction()
      if (!actionProcess.running && root.pendingActions.length === 0) root.refresh()
    }
  }

  Process {
    id: statusProcess
    command: ["salah", "status", "--json"]
    stdout: StdioCollector {
      id: statusStdout
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: statusStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      if (exitCode === 0) root.parseReport(statusStdout.text)
      else root.reportFailure(statusStderr.text)
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
      property alias quickField: quickField
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
          "title": "", "time": "", "state": "", "measure": "check", "target": 0,
          "minutes": 0, "count": 0, "repeat": "none", "timer_started_at": "",
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

          var blockItems = root.itemsForBlock(block, items)
          for (var i = 0; i < blockItems.length; i++) {
            var source = blockItems[i]
            var item = baseRow("item", "item:" + String(source.id), block)
            item.itemId = String(source.id || "")
            item.title = String(source.title || "")
            item.state = String(source.state || "open")
            item.measure = String(source.measure || "check")
            item.target = Number(source.target) || 0
            item.minutes = Number(source.minutes) || 0
            item.count = Number(source.count) || 0
            item.repeat = String(source.repeat || "none")
            item.timer_started_at = String(source.timer_started_at || "")
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
        var desired = desiredRows(status)
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
        for (var tail = timelineModel.count - 1; tail >= desired.length; tail--)
          timelineModel.remove(tail)
        if (expandedItemId !== "" && indexOfKey("item:" + expandedItemId) < 0)
          collapseExpanded()
      }

      function patchItem(id, changes) {
        var index = indexOfKey("item:" + id)
        if (index < 0) return
        for (var role in changes) timelineModel.setProperty(index, role, changes[role])
      }

      function removeItem(id) {
        var index = indexOfKey("item:" + id)
        if (index >= 0) timelineModel.remove(index)
      }

      function setRunningTimer(id) {
        var started = new Date().toISOString()
        for (var i = 0; i < timelineModel.count; i++) {
          var row = timelineModel.get(i)
          if (row.rowType === "item")
            timelineModel.setProperty(i, "timer_started_at", row.itemId === id ? started : "")
        }
      }

      function weekdayName(index) {
        var keys = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"]
        return strings.t("weekday." + keys[index])
      }

      function showNextOccurrence(raw) {
        try {
          var item = JSON.parse(String(raw || "").trim())
          var today = new Date()
          var occurs = false
          var next = ""
          if (item.repeat === "daily") occurs = true
          else if (item.repeat === "weekly" && item.weekdays instanceof Array) {
            var dayKeys = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"]
            occurs = item.weekdays.indexOf(dayKeys[today.getDay()]) >= 0
            for (var offset = occurs ? 0 : 1; !occurs && offset <= 7; offset++) {
              var candidate = (today.getDay() + offset) % 7
              if (item.weekdays.indexOf(dayKeys[candidate]) >= 0) {
                next = weekdayName(candidate); break
              }
            }
          } else if (item.day) {
            var parts = String(item.day).split("-")
            if (parts.length === 3) {
              var date = new Date(Number(parts[0]), Number(parts[1]) - 1, Number(parts[2]))
              occurs = date.toDateString() === today.toDateString()
              if (!occurs && date > today) next = weekdayName(date.getDay())
            }
          }
          if (!occurs && next !== "") {
            occurrenceHint = strings.t("panel.nextOccurrence").arg(String(item.title || "")).arg(next)
            occurrenceHintTimer.restart()
          }
        } catch (error) {
          // Status refresh remains authoritative if optional add metadata is malformed.
        }
      }

      function togglePrayerOptimistic(id) {
        var index = indexOfKey("header:" + id)
        if (index < 0) return false
        var marked = !timelineModel.get(index).marked
        timelineModel.setProperty(index, "marked", marked)
        return marked
      }

      function addPendingItem(title, requestedBlock) {
        var block = String(requestedBlock || "")
        if (block === "") block = root.planner ? String(root.planner.block || "") : ""
        if (["night", "fajr", "dhuhr", "asr", "maghrib", "isha"].indexOf(block) < 0)
          block = "fajr"
        var addIndex = indexOfKey("add:" + block)
        if (addIndex < 0) return
        var pending = baseRow("item", "pending:" + Date.now(), block)
        pending.title = title
        pending.itemId = pending.rowKey
        pending.pending = true
        timelineModel.insert(addIndex, pending)
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
        dragPointerY = Math.max(0, Math.min(timelineList.contentHeight, pointerY))
        var from = indexOfKey(dragKey)
        var hit = timelineList.indexAt(Style.space(10), dragPointerY)
        if (from < 0 || hit < 0) return
        var hitRow = timelineModel.get(hit)
        var slot = hit
        var delegate = timelineList.itemAtIndex(hit)
        if (hitRow.rowType === "header") slot = hit + 1
        else if (hitRow.rowType === "item" && delegate && dragPointerY > delegate.y + delegate.height / 2) slot = hit + 1
        else if (hitRow.rowType === "marker") {
          while (slot > 0 && timelineModel.get(slot).rowType === "marker") slot--
        }
        var target = slot > from ? slot - 1 : slot
        if (target !== from && target >= 0 && target < timelineModel.count)
          timelineModel.move(from, target, 1)
      }

      function restoreDragOrder() {
        for (var i = 0; i < dragSnapshot.length; i++) {
          var found = indexOfKey(dragSnapshot[i])
          if (found >= 0 && found !== i) timelineModel.move(found, i, 1)
        }
      }

      function cancelDrag() {
        if (dragKey === "") return
        restoreDragOrder()
        dragKey = ""
        dragSnapshot = []
      }

      function finishDrag(inside) {
        if (dragKey === "") return
        if (!inside) {
          cancelDrag()
          return
        }
        var itemIndex = indexOfKey(dragKey)
        var block = ""
        var blockIndex = 0
        for (var i = itemIndex - 1; i >= 0; i--) {
          var row = timelineModel.get(i)
          if (row.rowType === "header") { block = row.block; break }
          if (row.rowType === "item") blockIndex++
        }
        if (block === "") { cancelDrag(); return }
        var id = timelineModel.get(itemIndex).itemId
        timelineModel.setProperty(itemIndex, "block", block)
        dragKey = ""
        dragSnapshot = []
        root.runAction(["item", "move", "--json", id, "--block", block, "--index", String(blockIndex)])
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

          Column {
            width: parent.width
            spacing: Style.space(3)

            TextField {
              id: quickField
              width: parent.width
              placeholderText: strings.t("panel.quickAddPlaceholder")
              foreground: root.foreground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              maximumLength: 200

              onAccepted: {
                root.addItem(text)
                text = ""
              }

              Keys.onEscapePressed: function(event) {
                root.clearQuickAddPreset()
                text = ""
                focus = false
                event.accepted = true
              }
            }

            Text {
              visible: root.quickAddBlock !== ""
              width: parent.width
              textFormat: Text.PlainText
              text: strings.t("panel.addingTo").arg(root.prayerName(root.quickAddBlock, root.quickAddBlock))
              color: root.mutedForeground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.caption
              elide: Text.ElideRight
            }


            Text {
              visible: keyCatcher.occurrenceHint !== ""
              width: parent.width
              textFormat: Text.PlainText
              text: keyCatcher.occurrenceHint
              color: root.mutedForeground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.caption
              elide: Text.ElideRight
            }

            Text {
              visible: root.actionError !== ""
              width: parent.width
              textFormat: Text.PlainText
              text: root.actionError
              color: root.mutedForeground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.caption
              wrapMode: Text.WrapAnywhere
              maximumLineCount: 2
              elide: Text.ElideRight
            }
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

              delegate: Item {
                id: flatRow
                required property int index
                required property string rowType
                required property string rowKey
                required property string block
                required property string itemId
                required property string title
                required property string time
                required property string state
                required property string measure
                required property int target
                required property int minutes
                required property int count
                required property string repeat
                required property string timer_started_at
                required property bool current
                required property bool marked
                required property bool passed
                required property bool pending
                readonly property bool dragging: keyCatcher.dragKey === rowKey
                readonly property bool done: state === "done"
                readonly property bool expanded: rowType === "item"
                  && keyCatcher.expandedItemId === itemId
                property bool editing: false
                property bool confirmingDelete: false
                property string editText: title
                property string correctionError: ""
                onExpandedChanged: if (!expanded) {
                  editing = false
                  confirmingDelete = false
                  correctionError = ""
                }
                function focusTimeField() {
                  if (!expanded) return
                  correctionField.forceActiveFocus()
                  correctionField.selectAll()
                }
                function selectRow() {
                  timelineList.currentIndex = index
                  if (keyCatcher.expandedItemId !== ""
                      && keyCatcher.expandedItemId !== itemId)
                    keyCatcher.collapseExpanded()
                }
                function beginTitleEdit() {
                  if (repeat !== "none") return
                  editing = true
                  confirmingDelete = false
                  editText = title
                  keyCatcher.editingKey = rowKey
                  titleEditor.forceActiveFocus()
                  titleEditor.selectAll()
                }
                readonly property int elapsedMinutes: {
                  root.minuteTick
                  var started = Date.parse(timer_started_at)
                  if (isNaN(started)) return Math.max(0, minutes)
                  var thenMinutes = Math.max(0, Math.floor((root.timerSnapshotMs - started) / 60000))
                  var nowMinutes = Math.max(0, Math.floor((Date.now() - started) / 60000))
                  return Math.max(0, minutes + nowMinutes - thenMinutes)
                }
                readonly property var itemData: ({
                  "id": itemId, "title": title, "block": block, "state": state,
                  "measure": measure, "target": target, "minutes": minutes,
                  "count": count, "repeat": repeat, "timer_started_at": timer_started_at
                })
                width: ListView.view.width
                height: rowType === "header" ? Style.space(40)
                  : (rowType === "item" ? Style.space(expanded
                      ? (correctionError !== "" ? 102 : 84) : 42)
                    : (rowType === "marker" ? Style.space(24) : Style.space(27)))
                Behavior on height {
                  NumberAnimation { duration: Style.duration(120); easing.type: Easing.OutCubic }
                }
                opacity: dragging ? 0 : (pending ? 0.5 : (passed && !current ? 0.58 : 1))

                Rectangle {
                  visible: flatRow.rowType === "item" && flatRow.expanded
                  anchors.fill: parent
                  anchors.leftMargin: Style.space(28)
                  radius: Style.cornerRadius
                  color: Style.hoverFillFor(root.foreground, Commons.Color.accent)
                }

                Rectangle {
                  visible: flatRow.rowType === "header"
                  anchors.fill: parent
                  radius: Style.cornerRadius
                  color: flatRow.current
                    ? Style.selectedFillFor(root.foreground, Commons.Color.accent)
                    : (headerHover.containsMouse
                        ? Style.hoverFillFor(root.foreground, Commons.Color.accent)
                        : "transparent")

                  Rectangle {
                    id: prayerCircle
                    visible: flatRow.block !== "night"
                    anchors.left: parent.left
                    anchors.leftMargin: Style.space(8)
                    anchors.verticalCenter: parent.verticalCenter
                    width: Style.space(18)
                    height: width
                    radius: width / 2
                    color: flatRow.marked
                      ? Style.selectedStateColor(root.foreground, Commons.Color.accent) : "transparent"
                    border.width: Style.spacing.hairline
                    border.color: flatRow.marked
                      ? Style.selectedStateColor(root.foreground, Commons.Color.accent) : root.foreground

                    Text {
                      visible: flatRow.marked
                      anchors.centerIn: parent
                      textFormat: Text.PlainText
                      text: strings.t("panel.checkSymbol")
                      color: Commons.Color.background
                      font.family: root.bar ? root.bar.fontFamily : Style.font.family
                      font.pixelSize: Style.font.caption
                      font.bold: true
                    }

                    MouseArea {
                      anchors.fill: parent
                      anchors.margins: -Style.space(6)
                      cursorShape: Qt.PointingHandCursor
                      onClicked: root.togglePrayer({ "id": flatRow.block })
                    }
                  }

                  MouseArea {
                    id: headerHover
                    anchors.left: flatRow.block === "night" ? parent.left : prayerCircle.right
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    hoverEnabled: true
                    acceptedButtons: Qt.NoButton
                  }

                  Text {
                    anchors.left: parent.left
                    anchors.leftMargin: flatRow.block === "night" ? Style.space(8) : Style.space(36)
                    anchors.verticalCenter: parent.verticalCenter
                    textFormat: Text.PlainText
                    text: root.prayerName(flatRow.block, flatRow.title)
                    color: root.foreground
                    font.family: root.bar ? root.bar.fontFamily : Style.font.family
                    font.pixelSize: Style.font.body
                    font.bold: flatRow.current
                  }

                  Text {
                    anchors.right: nowTag.left
                    anchors.rightMargin: Style.space(8)
                    anchors.verticalCenter: parent.verticalCenter
                    textFormat: Text.PlainText
                    text: flatRow.time || strings.t("panel.noTimeSymbol")
                    color: root.mutedForeground
                    font.family: root.bar ? root.bar.fontFamily : Style.font.family
                    font.pixelSize: Style.font.bodySmall
                  }

                  Rectangle {
                    id: nowTag
                    visible: flatRow.current
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
                      text: strings.t("panel.now")
                      color: Style.selectedStateColor(root.foreground, Commons.Color.accent)
                      font.family: root.bar ? root.bar.fontFamily : Style.font.family
                      font.pixelSize: Style.font.caption
                      font.bold: true
                    }
                  }
                }

                Rectangle {
                  visible: flatRow.rowType === "item"
                  anchors.top: parent.top
                  anchors.left: parent.left
                  anchors.right: parent.right
                  anchors.leftMargin: Style.space(28)
                  height: Style.space(42)
                  radius: Style.cornerRadius
                  color: rowHover.containsMouse && !flatRow.expanded
                    ? Style.hoverFillFor(root.foreground, Commons.Color.accent) : "transparent"

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
                    text: strings.t("panel.dragHandle")
                    color: root.mutedForeground
                    font.family: root.bar ? root.bar.fontFamily : Style.font.family
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
                      flatRow.selectRow()
                      var point = mapToItem(timelineList.contentItem, mouse.x, mouse.y)
                      keyCatcher.beginDrag(flatRow.rowKey, flatRow.title, point.y)
                    }
                    onPositionChanged: function(mouse) {
                      if (!activeDrag) return
                      var point = mapToItem(timelineList.contentItem, mouse.x, mouse.y)
                      keyCatcher.moveDrag(point.y)
                    }
                    onReleased: function(mouse) {
                      var point = mapToItem(timelineList.contentItem, mouse.x, mouse.y)
                      var inside = point.x >= 0 && point.x <= timelineList.width
                        && point.y >= 0 && point.y <= timelineList.contentHeight
                      activeDrag = false
                      keyCatcher.finishDrag(inside)
                    }
                    onCanceled: { activeDrag = false; keyCatcher.cancelDrag() }
                  }

                  Rectangle {
                    id: itemCheck
                    anchors.left: parent.left
                    anchors.leftMargin: Style.space(30)
                    anchors.verticalCenter: parent.verticalCenter
                    width: Style.space(15)
                    height: width
                    radius: Math.min(Style.cornerRadius, Style.space(3))
                    color: flatRow.done
                      ? Style.selectedStateColor(root.foreground, Commons.Color.accent) : "transparent"
                    border.width: Style.spacing.hairline
                    border.color: flatRow.done
                      ? Style.selectedStateColor(root.foreground, Commons.Color.accent) : root.mutedForeground

                    Text {
                      visible: flatRow.done
                      anchors.centerIn: parent
                      textFormat: Text.PlainText
                      text: strings.t("panel.checkSymbol")
                      color: Commons.Color.background
                      font.family: root.bar ? root.bar.fontFamily : Style.font.family
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
                      timelineList.currentIndex = flatRow.index
                      flatRow.forceActiveFocus()
                      if (keyCatcher.expandedItemId !== ""
                          && keyCatcher.expandedItemId !== flatRow.itemId)
                        keyCatcher.collapseExpanded()
                    }
                    onPressAndHold: function(mouse) {
                      draggedThisPress = true
                      var point = mapToItem(timelineList.contentItem, mouse.x, mouse.y)
                      keyCatcher.beginDrag(flatRow.rowKey, flatRow.title, point.y)
                    }
                    onPositionChanged: function(mouse) {
                      if (!draggedThisPress) return
                      var point = mapToItem(timelineList.contentItem, mouse.x, mouse.y)
                      keyCatcher.moveDrag(point.y)
                    }
                    onReleased: function(mouse) {
                      if (!draggedThisPress) return
                      var point = mapToItem(timelineList.contentItem, mouse.x, mouse.y)
                      var inside = point.x >= 0 && point.x <= timelineList.width
                        && point.y >= 0 && point.y <= timelineList.contentHeight
                      keyCatcher.finishDrag(inside)
                    }
                    onCanceled: if (draggedThisPress) keyCatcher.cancelDrag()
                    onClicked: if (!draggedThisPress && !flatRow.pending) root.toggleItem(flatRow.itemData)
                  }

                  Column {
                    anchors.left: itemCheck.right
                    anchors.leftMargin: Style.space(9)
                    anchors.right: controlRow.left
                    anchors.rightMargin: Style.space(6)
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 0

                    Text {
                      visible: !flatRow.editing
                      width: parent.width
                      textFormat: Text.PlainText
                      text: flatRow.title
                      color: flatRow.done || flatRow.pending ? root.mutedForeground : root.foreground
                      font.family: root.bar ? root.bar.fontFamily : Style.font.family
                      font.pixelSize: Style.font.bodySmall
                      font.strikeout: flatRow.done
                      elide: Text.ElideRight
                    }

                    TextField {
                      id: titleEditor
                      visible: flatRow.editing
                      width: parent.width
                      height: Style.space(26)
                      foreground: root.foreground
                      font.family: root.bar ? root.bar.fontFamily : Style.font.family
                      font.pixelSize: Style.font.bodySmall
                      horizontalPadding: Style.space(4)
                      verticalPadding: 0
                      maximumLength: 200
                      text: flatRow.editText
                      onAccepted: {
                        root.editItem(flatRow.itemData, text)
                        flatRow.editing = false
                        keyCatcher.editingKey = ""
                        keyCatcher.forceActiveFocus()
                      }
                      Keys.onEscapePressed: function(event) {
                        flatRow.editing = false
                        keyCatcher.editingKey = ""
                        keyCatcher.forceActiveFocus()
                        event.accepted = true
                      }
                    }

                    Text {
                      visible: !flatRow.editing && text !== ""
                      width: parent.width
                      textFormat: Text.PlainText
                      text: flatRow.pending ? strings.t("panel.pending")
                        : root.itemDetail(flatRow.itemData)
                      color: root.mutedForeground
                      font.family: root.bar ? root.bar.fontFamily : Style.font.family
                      font.pixelSize: Style.font.caption
                      elide: Text.ElideRight
                    }
                  }

                  Row {
                    id: controlRow
                    visible: !flatRow.pending && !flatRow.editing
                    anchors.right: parent.right
                    anchors.rightMargin: Style.space(3)
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Style.space(1)

                    Row {
                      visible: flatRow.measure === "count"
                      height: Style.space(26)
                      spacing: 0

                      Button {
                        width: Style.space(24)
                        height: parent.height
                        text: strings.t("panel.decrease")
                        tooltipText: strings.t("panel.decreaseCount")
                        foreground: root.mutedForeground
                        fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
                        horizontalPadding: 0
                        verticalPadding: 0
                        onClicked: {
                          flatRow.selectRow()
                          root.adjustCount(flatRow.itemData, -1)
                        }
                      }
                      Row {
                        width: flatRow.target > 0 ? Style.space(54) : Style.space(32)
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: Style.space(2)

                        Text {
                          anchors.verticalCenter: parent.verticalCenter
                          textFormat: Text.PlainText
                          text: String(flatRow.count)
                          color: root.foreground
                          font.family: "monospace"
                          font.pixelSize: Style.font.caption
                        }
                        Text {
                          visible: flatRow.target > 0
                          anchors.verticalCenter: parent.verticalCenter
                          textFormat: Text.PlainText
                          text: strings.t("panel.countGoal").arg(flatRow.target)
                          color: root.mutedForeground
                          font.family: "monospace"
                          font.pixelSize: Style.font.caption
                        }
                      }
                      Button {
                        width: Style.space(24)
                        height: parent.height
                        text: strings.t("panel.increase")
                        tooltipText: strings.t("panel.increaseCount")
                        foreground: root.mutedForeground
                        fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
                        horizontalPadding: 0
                        verticalPadding: 0
                        onClicked: {
                          flatRow.selectRow()
                          root.adjustCount(flatRow.itemData, 1)
                        }
                      }
                    }
                    Button {
                      width: Style.space(68)
                      height: Style.space(26)
                      text: (flatRow.timer_started_at !== ""
                          ? strings.t("panel.pauseWithTime") : strings.t("panel.startWithTime"))
                        .arg(root.formatMinutes(flatRow.elapsedMinutes))
                      tooltipText: flatRow.timer_started_at !== ""
                        ? strings.t("panel.pauseTimer") : strings.t("panel.startTimer")
                      foreground: root.foreground
                      fontFamily: "monospace"
                      horizontalPadding: 0
                      verticalPadding: 0
                      bordered: true
                      onClicked: {
                        flatRow.selectRow()
                        root.toggleTimer(flatRow.itemData)
                      }
                    }
                    Button {
                      width: Style.space(30)
                      height: Style.space(28)
                      text: strings.t("panel.moreSymbol")
                      tooltipText: strings.t("panel.moreActions")
                      foreground: root.mutedForeground
                      fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
                      horizontalPadding: 0
                      verticalPadding: 0
                      onClicked: {
                        timelineList.currentIndex = flatRow.index
                        keyCatcher.toggleExpanded(flatRow.itemId)
                      }
                    }
                  }
                }

                Item {
                  visible: flatRow.rowType === "item" && flatRow.expanded
                  anchors.left: parent.left
                  anchors.leftMargin: Style.space(58)
                  anchors.right: parent.right
                  anchors.rightMargin: Style.space(3)
                  anchors.top: parent.top
                  anchors.topMargin: Style.space(44)
                  height: Style.space(38)

                  Row {
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Style.space(4)

                    Text {
                      anchors.verticalCenter: parent.verticalCenter
                      textFormat: Text.PlainText
                      text: strings.t("panel.time")
                      color: root.mutedForeground
                      font.family: root.bar ? root.bar.fontFamily : Style.font.family
                      font.pixelSize: Style.font.caption
                    }
                    TextField {
                      id: correctionField
                      width: Style.space(78)
                      height: Style.space(28)
                      placeholderText: strings.t("panel.correctionPlaceholder")
                      foreground: root.foreground
                      font.family: "monospace"
                      font.pixelSize: Style.font.caption
                      horizontalPadding: Style.space(5)
                      verticalPadding: 0
                      maximumLength: 5
                      onActiveFocusChanged: {
                        if (activeFocus) keyCatcher.editingKey = flatRow.rowKey
                        else if (keyCatcher.editingKey === flatRow.rowKey && !titleEditor.activeFocus)
                          keyCatcher.editingKey = ""
                      }
                      onAccepted: {
                        var raw = String(text).trim()
                        var value = Number(raw)
                        if (!/^[+-]?\d+$/.test(raw) || value === 0 || value < -1440 || value > 1440) {
                          flatRow.correctionError = strings.t("panel.correctionError")
                          return
                        }
                        root.correctMinutes(flatRow.itemData, value)
                        text = ""
                        flatRow.correctionError = ""
                      }
                      Keys.onEscapePressed: function(event) {
                        text = ""
                        flatRow.correctionError = ""
                        keyCatcher.collapseExpanded()
                        keyCatcher.forceActiveFocus()
                        event.accepted = true
                      }
                    }
                    Repeater {
                      model: [
                        { "amount": -5, "key": "panel.minusFive" },
                        { "amount": 5, "key": "panel.plusFive" },
                        { "amount": 15, "key": "panel.plusFifteen" }
                      ]
                      Button {
                        required property var modelData
                        width: Style.space(34)
                        height: Style.space(28)
                        text: strings.t(modelData.key)
                        foreground: root.mutedForeground
                        fontFamily: "monospace"
                        horizontalPadding: 0
                        verticalPadding: 0
                        onClicked: root.correctMinutes(flatRow.itemData, modelData.amount)
                      }
                    }
                    Button {
                      visible: flatRow.repeat === "none" && !flatRow.confirmingDelete
                      width: Style.space(42)
                      height: Style.space(28)
                      text: strings.t("panel.edit")
                      foreground: root.mutedForeground
                      fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
                      horizontalPadding: Style.space(3)
                      verticalPadding: 0
                      onClicked: flatRow.beginTitleEdit()
                    }
                    Button {
                      visible: flatRow.repeat === "none" && !flatRow.confirmingDelete
                      width: Style.space(52)
                      height: Style.space(28)
                      text: strings.t("panel.delete")
                      foreground: root.mutedForeground
                      fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
                      horizontalPadding: Style.space(3)
                      verticalPadding: 0
                      onClicked: flatRow.confirmingDelete = true
                    }
                    Text {
                      visible: flatRow.repeat !== "none"
                      anchors.verticalCenter: parent.verticalCenter
                      textFormat: Text.PlainText
                      text: strings.t("panel.editRoutinesInApp")
                      color: root.mutedForeground
                      font.family: root.bar ? root.bar.fontFamily : Style.font.family
                      font.pixelSize: Style.font.caption
                    }
                    Text {
                      visible: flatRow.confirmingDelete
                      anchors.verticalCenter: parent.verticalCenter
                      textFormat: Text.PlainText
                      text: strings.t("panel.confirmDelete")
                      color: root.mutedForeground
                      font.family: root.bar ? root.bar.fontFamily : Style.font.family
                      font.pixelSize: Style.font.caption
                    }
                    Button {
                      visible: flatRow.confirmingDelete
                      width: Style.space(32)
                      height: Style.space(28)
                      text: strings.t("panel.yes")
                      foreground: root.foreground
                      horizontalPadding: 0
                      verticalPadding: 0
                      onClicked: {
                        keyCatcher.collapseExpanded()
                        root.removeItem(flatRow.itemData)
                      }
                    }
                    Button {
                      visible: flatRow.confirmingDelete
                      width: Style.space(32)
                      height: Style.space(28)
                      text: strings.t("panel.no")
                      foreground: root.foreground
                      horizontalPadding: 0
                      verticalPadding: 0
                      onClicked: flatRow.confirmingDelete = false
                    }
                  }

                  Text {
                    visible: flatRow.correctionError !== ""
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.bottom
                    textFormat: Text.PlainText
                    text: flatRow.correctionError
                    color: root.mutedForeground
                    font.family: root.bar ? root.bar.fontFamily : Style.font.family
                    font.pixelSize: Style.font.caption
                    elide: Text.ElideRight
                  }
                }

                Button {
                  visible: flatRow.rowType === "add"
                  anchors.left: parent.left
                  anchors.leftMargin: Style.space(34)
                  anchors.verticalCenter: parent.verticalCenter
                  height: Style.space(25)
                  text: strings.t("panel.addRow").arg(root.prayerName(flatRow.block, flatRow.block))
                  tooltipText: strings.t("panel.addTo").arg(root.prayerName(flatRow.block, flatRow.block))
                  foreground: root.mutedForeground
                  fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
                  horizontalPadding: Style.space(5)
                  verticalPadding: Style.space(1)
                  onClicked: root.presetQuickAdd(flatRow.block)
                }

                Item {
                  visible: flatRow.rowType === "marker"
                  anchors.fill: parent

                  Text {
                    anchors.left: parent.left
                    anchors.leftMargin: Style.space(38)
                    anchors.verticalCenter: parent.verticalCenter
                    textFormat: Text.PlainText
                    text: flatRow.rowKey === "marker:sunrise"
                      ? strings.t("boundary.sunrise") : strings.t("boundary.sleep")
                    color: root.mutedForeground
                    font.family: root.bar ? root.bar.fontFamily : Style.font.family
                    font.pixelSize: Style.font.caption
                  }
                  Text {
                    anchors.right: parent.right
                    anchors.rightMargin: Style.space(10)
                    anchors.verticalCenter: parent.verticalCenter
                    textFormat: Text.PlainText
                    text: flatRow.time
                    color: root.mutedForeground
                    font.family: root.bar ? root.bar.fontFamily : Style.font.family
                    font.pixelSize: Style.font.caption
                  }
                }
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
