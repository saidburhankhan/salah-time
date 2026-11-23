import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "CreationLogic.js" as CreationLogic
import "DragLogic.js" as DragLogic
import "ResourceLogic.js" as ResourceLogic
import "SessionLogic.js" as SessionLogic

BarWidget {
  id: root
  moduleName: "local.salah"

  Commons.Translations {
    id: strings
    directory: Qt.resolvedUrl("i18n")
  }

  readonly property var tr: strings
  readonly property alias resourceClient: client
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
  property var pendingOps: ({})
  property var dueReminders: []
  property var pendingReminders: ({})
  property string reminderError: ""
  property var projects: []
  property var epics: []
  property var metricCatalogue: []
  property string sessionPanelMode: ""
  property var doneItem: null
  property string pauseId: ""
  property string sessionPanelOp: ""
  property string sessionPanelError: ""
  property string noteSource: ""
  property string noteSessionId: ""
  property string noteDraft: ""
  property string noteOp: ""
  property string noteError: ""
  property var noteSubmission: null
  // An error from a row's own action, shown inside that expanded row.
  property string rowErrorKey: ""
  property string rowErrorText: ""

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
  readonly property var runningTimer: report && report.running ? report.running : null
  readonly property bool canAnnotateTimer: SessionLogic.canAnnotateRunning(runningTimer)
  readonly property int workedMinutes: runningTimer
    ? SessionLogic.workedMinutes(runningTimer.started_at, countdownNowMs, runningTimer.max_minutes) : 1
  readonly property real distractions: runningTimer && runningTimer.values
    ? Number(runningTimer.values.distractions || 0) : 0
  readonly property var runningSeconds: runningTimer
    ? SessionLogic.runningDisplaySeconds(runningTimer, countdownNowMs) : null
  readonly property string runningElapsedText: !runningTimer ? ""
    : runningSeconds === null ? strings.t("panel.noTimeSymbol") : SessionLogic.clockText(runningSeconds)

  readonly property var runningItem: runningTimer ? plannerItem(runningTimer.item_id) : null
  // Everything worked on the running task today: its finished sessions plus
  // the running one, in whole minutes (the hover tooltip shows this).
  readonly property int runningTotalMinutes: (runningItem ? Number(runningItem.actual_minutes || 0) : 0)
    + Math.floor((runningSeconds || 0) / 60)
  readonly property var runningProject: runningTimer
    ? SessionLogic.findProject(projects, runningTimer.project_id) : null
  readonly property string runningTitle: runningItem ? String(runningItem.title || "")
    : (runningProject ? String(runningProject.name || "") : strings.t("session.untitled"))
  readonly property color runningColor: {
    if (runningItem && runningItem.project && runningItem.project.color)
      return String(runningItem.project.color)
    if (runningProject && runningProject.color) return String(runningProject.color)
    return Commons.Color.accent
  }
  readonly property string reminderBadge: dueReminders.length
    ? strings.t("bar.reminderBadge").arg(Commons.I18n.formatNumber(dueReminders.length)) : ""
  readonly property string prayerStatus: localizedNextName !== ""
    ? strings.t("bar.nextStatus").arg(localizedNextName).arg(remainingShort).arg("")
    : (firstLoad ? strings.t("bar.loading") : strings.t("bar.unavailableShort"))
  readonly property string barPrefix: reminderBadge ? prayerStatus + "  " + reminderBadge : prayerStatus
  // Three en spaces hold the running session's colour dot, painted over the
  // middle one, then the elapsed time.
  readonly property string runningGap: "\u2002\u2002\u2002"
  readonly property string displayText: runningTimer
    ? barPrefix + runningGap + strings.t("bar.runningTimer").arg(runningElapsedText)
    : barPrefix
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

  function formatDuration(value) {
    var total = Math.max(0, Math.floor(Number(value) || 0))
    if (total < 60) return formatMinutes(total)
    var minutes = total % 60
    return strings.t("unit.hoursMinutesShort").arg(Math.floor(total / 60))
      .arg(minutes < 10 ? "0" + minutes : String(minutes))
  }

  function resourceDateTime(value) {
    var date = new Date(value)
    if (!value || isNaN(date.getTime())) return ""
    return date.toLocaleString(Qt.locale(Commons.I18n.formatLocale), Locale.ShortFormat)
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

  function beginOp(kind, rowKey, tempKey, retryId) {
    actionError = ""
    if (rowErrorKey === String(rowKey || "")) {
      rowErrorKey = ""
      rowErrorText = ""
    }
    var opId = retryId || client.newOpId()
    var entry = { "kind": String(kind || ""), "rowKey": String(rowKey || "") }
    if (tempKey) entry.tempKey = String(tempKey)
    pendingOps[opId] = entry
    return opId
  }

  function clearPending(opId) {
    var pending = pendingOps[opId]
    if (!pending) return
    delete pendingOps[opId]
    if (pending.reminderId) {
      var remaining = Object.assign({}, pendingReminders)
      delete remaining[pending.reminderId]
      pendingReminders = remaining
    }
    var content = panel.loadedContent
    if (content && pending.rowKey) content.setRowPending(pending.rowKey, false)
    if (content && pending.tempKey) content.removeItem(pending.tempKey)
  }

  function completeReminder(id, itemId, version) {
    if (!id || pendingReminders[id]) return
    if (!itemId) reminderError = ""
    var opId = beginOp("resource.edit", itemId ? "item:" + itemId : "")
    pendingOps[opId].reminderId = id
    var next = Object.assign({}, pendingReminders)
    next[id] = true
    pendingReminders = next
    var params = { "id": id, "done": true }
    if (version) params.expected_version = version
    client.rpcMutation(opId, "resource.edit", params)
  }

  function addResource(opId, params) {
    beginOp("resource.add", "item:" + params.item_id, "", opId)
    client.rpcMutation(opId, "resource.add", params)
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
      content.revealQuickField()
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

  function plannerItem(id) {
    var key = String(id || "")
    var items = planner && planner.items ? planner.items : []
    if (key === "") return null
    for (var i = 0; i < items.length; i++)
      if (String(items[i].id || "") === key) return items[i]
    return null
  }

  // `status` is passed only so a binding re-reads after each report.
  function itemSessions(id, status) {
    var item = plannerItem(id)
    return item && item.sessions ? item.sessions : []
  }

  // Done box (plan.md, user decisions): an item with a tick is unticked
  // (its tick sessions removed, timed sessions kept); otherwise the done
  // panel opens to tick it with optional metrics.
  function toggleItem(row) {
    var rowKey = "item:" + String(row.id)
    var item = plannerItem(row.id)
    var decision = SessionLogic.doneAction(item)
    if (decision.action === "untick") {
      for (var i = 0; i < decision.ids.length; i++)
        client.removeSession(beginOp("untick", rowKey), "untick", decision.ids[i])
    } else if (item) {
      openDonePanel(item)
    }
  }

  function toggleTimer(row) {
    if (runningTimer && String(runningTimer.item_id || "") === String(row.id)) {
      pauseRunning()
      return
    }
    var opId = beginOp("startSession", "item:" + String(row.id))
    client.startSession(opId, String(row.id), "")
  }

  function pauseRunning() {
    if (!runningTimer || sessionPanelOp !== "") return
    if (sessionPanelMode !== "pause" || pauseId !== runningTimer.id) {
      pauseId = runningTimer.id
      sessionPanelMode = "pause"
      sessionPanelError = ""
      sessionEditorReset("")
      refresh()
    }
    var content = panel.loadedContent
    if (content) Qt.callLater(content.revealSessionEditor)
  }

  function savePause(params) {
    if (sessionPanelOp || !runningTimer || params.id !== pauseId || params.id !== runningTimer.id) return
    sessionPanelError = ""
    var opId = beginOp("stopSession", runningTimer.item_id ? "item:" + runningTimer.item_id : "")
    sessionPanelOp = opId
    client.stopSession(opId, params)
  }

  function bump(delta, fromBar) {
    if (!canAnnotateTimer) return
    var opId = beginOp("bump", "")
    pendingOps[opId].pulse = fromBar
    client.rpcMutation(opId, "session.bump", {
      "id": runningTimer.id, "metric_id": "distractions", "delta": delta
    })
  }

  function openNote(source) {
    if (!canAnnotateTimer || noteOp) return
    if (noteSessionId !== runningTimer.id) noteDraft = ""
    noteSessionId = runningTimer.id
    noteError = ""
    noteSource = source
  }

  function closeNote() {
    noteSource = ""
    noteSessionId = ""
    noteDraft = ""
    noteError = ""
    noteOp = ""
    noteSubmission = null
  }

  function saveNote() {
    if (!noteDraft.trim() || noteOp || !canAnnotateTimer || runningTimer.id !== noteSessionId) return
    noteError = ""
    noteSubmission = SessionLogic.noteAttempt(noteSubmission, noteSessionId, noteDraft, client.newOpId)
    noteOp = beginOp("note", "", "", noteSubmission.opId)
    client.rpcMutation(noteOp, "session.note", Object.assign({}, noteSubmission.params))
  }

  function openDonePanel(item) {
    if (sessionPanelOp !== "") return
    doneItem = item
    sessionPanelMode = "done"
    sessionPanelOp = ""
    sessionPanelError = ""
    client.loadProjects()
    client.loadEpics()
    client.loadMetrics()
    sessionEditorReset("")
  }

  function openLogPanel(presetKey) {
    if (sessionPanelOp !== "") return
    doneItem = null
    sessionPanelMode = "log"
    sessionPanelOp = ""
    sessionPanelError = ""
    client.loadProjects()
    client.loadEpics()
    client.loadMetrics()
    sessionEditorReset(presetKey)
  }

  function closeSessionPanel() {
    sessionPanelMode = ""
    sessionPanelOp = ""
    sessionPanelError = ""
    doneItem = null
    pauseId = ""
  }

  function sessionEditorReset(presetKey) {
    var content = panel.loadedContent
    if (content && content.sessionEditor) content.sessionEditor.reset(presetKey)
  }

  // A duration-free log ticks the item and stops its matching timer atomically.
  function tickDone(item, values, note) {
    if (!item || sessionPanelOp !== "") return
    sessionPanelError = ""
    var id = String(item.id || "")
    sessionPanelOp = beginOp("tick", "item:" + id)
    client.logSession(sessionPanelOp, id, "", null, values, note, item.block)
  }

  function logTime(target, minutes, values, note, epicId) {
    if (!target || sessionPanelOp !== "") return
    sessionPanelError = ""
    var opId = beginOp("logSession", target.kind === "item" ? "item:" + String(target.id) : "")
    sessionPanelOp = opId
    if (target.kind === "item") client.logSession(opId, String(target.id), "", minutes, values, note, "")
    else client.logSession(opId, "", String(target.id), minutes, values, note, "", String(epicId || ""))
  }

  function removeSession(row, sessionId) {
    var opId = beginOp("removeSession", "item:" + String(row.id))
    client.removeSession(opId, "removeSession", sessionId)
  }

  function editSessionMinutes(row, sessionId, minutes) {
    var opId = beginOp("editSession", "item:" + String(row.id))
    client.editSessionMinutes(opId, sessionId, minutes)
  }

  function openApp() {
    // A fixed argv, no shell. The app opens on its own pages.
    Quickshell.execDetached(["salah-app"])
    close()
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
    if (noteSource === "card") closeNote()
    if (!sessionPanelOp) closeSessionPanel()
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
      var reminders = ResourceLogic.orderedDueReminders(status.reminders)
      if (JSON.stringify(reminders) !== JSON.stringify(dueReminders)) dueReminders = reminders
      var runningId = status.running ? status.running.id : ""
      if (sessionPanelMode === "pause" && pauseId !== runningId) closeSessionPanel()
      if (noteSource && (noteSessionId !== runningId || !canAnnotateTimer)) closeNote()
      // The item being ticked was removed elsewhere (the app, the CLI).
      if (sessionPanelMode === "done" && doneItem && sessionPanelOp === "") {
        var stillThere = null
        var items = status.planner && status.planner.items ? status.planner.items : []
        for (var i = 0; i < items.length; i++)
          if (String(items[i].id || "") === String(doneItem.id || "")) stillThere = items[i]
        if (stillThere) doneItem = stillThere
        else closeSessionPanel()
      }
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

  function handleMutationSuccess(opId, kind, result) {
    var reminderId = pendingOps[opId] && pendingOps[opId].reminderId
    if (reminderId)
      dueReminders = dueReminders.filter(function(row) { return row.id !== reminderId })
    if (pendingOps[opId] && pendingOps[opId].pulse) bumpPulse.restart()
    if (opId === noteOp) closeNote()
    if (opId === sessionPanelOp) closeSessionPanel()
    if (kind === "add") {
      var item = CreationLogic.addedItem(result)
      var target = panel.loadedContent
      if (target && item) target.showNextOccurrence(item)
    }
    clearPending(opId)
    client.refreshStatus()
  }

  // Daemon errors are shown where the action was taken: inside the session
  // panel, inside an expanded row, or under the quick-add field.
  function handleMutationFailure(opId, kind, message, data) {
    var pending = pendingOps[opId] || {}
    clearPending(opId)
    var detail = String(message || "").trim().replace(/[\r\n]+/g, " ")
    detail = detail === "" ? strings.t("panel.commandFailed") : detail.slice(0, 240)
    var content = panel.loadedContent
    if (pending.reminderId && !pending.rowKey) {
      reminderError = detail
    } else if (opId === noteOp) {
      noteOp = ""
      noteError = detail
    } else if (opId === sessionPanelOp) {
      sessionPanelOp = ""
      sessionPanelError = detail
    } else if (pending.rowKey && content
        && "item:" + content.expandedItemId === pending.rowKey) {
      rowErrorKey = pending.rowKey
      rowErrorText = detail
    } else {
      actionError = detail
    }
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
    // A reminder can become due without a database write or prayer boundary.
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

  SalahClient {
    id: client
    onStatusLoaded: function(status) { root.acceptReport(status) }
    onStatusFailed: function(message) { root.reportFailure(message) }
    onMutationSucceeded: function(opId, kind, result) { root.handleMutationSuccess(opId, kind, result) }
    onMutationFailed: function(opId, kind, message, data) { root.handleMutationFailure(opId, kind, message, data) }
    onProjectsLoaded: function(rows) { root.projects = rows || [] }
    onEpicsLoaded: function(rows) { root.epics = rows || [] }
    onMetricsLoaded: function(rows) { root.metricCatalogue = rows || [] }
    onEntitiesChanged: function(scopes) {
      if (scopes.indexOf("projects") >= 0 || scopes.indexOf("tags") >= 0
          || scopes.indexOf("metrics") >= 0) client.loadProjects()
      if (scopes.indexOf("metrics") >= 0) client.loadMetrics()
      if (scopes.indexOf("epics") >= 0 || scopes.indexOf("projects") >= 0) client.loadEpics()
    }
    onTransportChanged: function(active) {
      if (!active) root.refresh()
      else {
        client.loadProjects()
        client.loadEpics()
        client.loadMetrics()
      }
    }
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
    : (root.runningTimer ? Math.max(Style.space(250), button.implicitWidth + bumpButton.implicitWidth)
      : Math.max(Style.space(180), button.implicitWidth))
  implicitHeight: button.implicitHeight + (root.vertical && root.runningTimer ? bumpButton.implicitHeight : 0)

  WidgetButton {
    id: button
    x: !root.vertical && root.runningTimer ? (root.width - width - bumpButton.width) / 2 : 0
    width: !root.vertical && root.runningTimer ? implicitWidth : root.width
    height: root.vertical ? implicitHeight : root.height
    bar: root.bar
    text: root.vertical
      ? strings.t("bar.symbol") + (root.reminderBadge ? "\n" + root.reminderBadge : "")
      : root.displayText
    labelVisible: true
    horizontalMargin: root.runningTimer ? 2 : 8.75
    verticalPadding: 8.75
    tooltipText: (root.localizedNextName !== ""
      ? strings.t("bar.tooltip").arg(root.localizedNextName).arg(root.nextTime).arg(root.remainingLong)
      : strings.t("bar.unavailable"))
      + (root.dueReminders.length
        ? "\n" + strings.tn("reminder.count", root.dueReminders.length)
          .arg(Commons.I18n.formatNumber(root.dueReminders.length)) : "")
      + (root.runningTimer
        ? "\n" + strings.t("bar.runningTooltip").arg(root.runningTitle).arg(root.runningTotalMinutes) : "")
    pressFeedbackButtons: Qt.NoButton
    onPressed: function(mouseButton) {
      if (mouseButton === Qt.MiddleButton) root.syncLocation(true)
      else if (mouseButton === Qt.LeftButton) root.toggle()
    }
  }

  WidgetButton {
    id: bumpButton
    visible: root.runningTimer !== null
    bar: root.bar
    x: root.vertical ? 0 : button.x + button.width
    y: root.vertical ? button.height : 0
    width: root.vertical ? root.width : implicitWidth
    height: root.vertical ? implicitHeight : root.height
    horizontalMargin: 5
    verticalPadding: 2
    text: "+"
    tooltipText: strings.t("session.bumpHint")
    interactive: root.canAnnotateTimer
    dimmed: !root.canAnnotateTimer
    active: bumpPulse.running
    activeColor: Commons.Color.accent
    onPressed: function(mouseButton) {
      if (!root.canAnnotateTimer || (mouseButton !== Qt.LeftButton && mouseButton !== Qt.RightButton)) return
      root.bump(1, true)
      if (mouseButton === Qt.RightButton) root.openNote("bar")
    }
  }

  Timer {
    id: bumpPulse
    interval: 220
  }

  LazyLoader {
    active: root.noteSource === "bar"
    SessionNotePopover {
      host: root
      anchorItem: bumpButton
    }
  }

  TextMetrics {
    id: barPrefixMetrics
    font.family: button.fontFamily
    font.pixelSize: button.fontSize
    text: root.barPrefix + "\u2002"
  }

  TextMetrics {
    id: barGapMetrics
    font.family: button.fontFamily
    font.pixelSize: button.fontSize
    text: "\u2002"
  }

  // The running session's project colour, painted into the gap the label
  // leaves for it (the label itself is one plain-text colour). A vertical
  // bar shows only the dot, in the corner of the symbol.
  Rectangle {
    readonly property real size: Math.max(5, Math.round(button.fontSize * 0.55))
    visible: root.runningTimer !== null && button.visible
    width: size
    height: size
    radius: size / 2
    color: root.runningColor
    x: root.vertical
      ? button.width - size - Style.space(3)
      : Math.round(button.x + (button.width - button.labelWidth) / 2 + barPrefixMetrics.advanceWidth
        + (barGapMetrics.advanceWidth - size) / 2)
    y: root.vertical ? Style.space(3) : Math.round((button.height - size) / 2)
  }

  SalahKeyboardPanel {
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
      property alias sessionEditor: sessionEditor
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
      readonly property real maxContentHeight: Math.max(Style.space(240), panel.availableCardHeight - panel.padding * 2)
      implicitHeight: Math.min(dayColumn.implicitHeight, maxContentHeight)
      blocked: quickField.activeFocus || editingKey !== "" || sessionEditor.textFocus || runningCard.noteFocus
      function revealSessionEditor() {
        if (root.sessionPanelMode === "pause")
          dayScroll.contentY = Math.max(0, Math.min(sessionEditor.y, dayScroll.contentHeight - dayScroll.height))
      }
      function revealQuickField() {
        quickAdd.blockChoice = ""
        quickAdd.whenChoice = ""
        if (quickAdd.y < dayScroll.contentY || quickAdd.y + quickField.height > dayScroll.contentY + dayScroll.height)
          dayScroll.contentY = Math.min(quickAdd.y, Math.max(0, dayScroll.contentHeight - dayScroll.height))
      }
      readonly property string currentBlock: root.planner ? root.planner.block : ""
      onCurrentBlockChanged: if (quickAdd && quickAdd.expanded) Qt.callLater(keyCatcher.revealCreationBlock)

      // While composing, the timeline has its own viewport directly below
      // the sheet. Keep the active block in view even late in a busy day.
      function revealCreationBlock() {
        if (!quickAdd.expanded) {
          timelineList.positionViewAtBeginning()
          return
        }
        dayColumn.forceLayout()
        var index = indexOfKey("header:" + currentBlock)
        if (index >= 0) timelineList.positionViewAtIndex(index, ListView.Beginning)
        var bottom = flatTimeline.y + Math.min(flatTimeline.height, Style.space(96))
        dayScroll.contentY = Math.min(quickAdd.y, Math.max(0, dayScroll.contentHeight - dayScroll.height),
          Math.max(dayScroll.contentY, bottom - dayScroll.height))
      }
      function collapseExpanded() {
        expandedItemId = ""
        editingKey = ""
      }

      function toggleExpanded(id) {
        expandedItemId = expandedItemId === id ? "" : id
        editingKey = ""
      }

      onCloseRequested: {
        if (dragKey !== "") cancelDrag()
        else if (quickAdd.expanded) quickAdd.dismiss()
        else if (root.sessionPanelMode !== "") root.closeSessionPanel()
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
          "title": "", "time": "", "itemState": "", "projectId": "", "projectColor": "transparent", "epicName": "", "plannedMinutes": 0,
          "minutes": 0, "count": 0, "resourceCount": 0, "hasResourceSupport": false,
          "repeat": "none", "timerRunning": false,
          "detailText": "",
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
            var item = baseRow("item", "item:" + String(source.id), block)
            item.itemId = String(source.id || "")
            item.title = String(source.title || "")
            item.itemState = source.done === true ? "done" : "open"
            item.projectId = source.project ? String(source.project.id || "") : ""
            item.projectColor = source.project ? String(source.project.color || "transparent") : "transparent"
            item.epicName = CreationLogic.epicName(root.epics, String(source.epic_id || ""))
            item.plannedMinutes = Number(source.planned_minutes) || 0
            item.minutes = Number(source.actual_minutes) || 0
            item.hasResourceSupport = !!source.resources
            item.resourceCount = ResourceLogic.resourceCount(source.resources)
            item.repeat = String(source.repeat || "none")
            item.timerRunning = status.running !== null && status.running !== undefined
              && String(status.running.item_id || "") === item.itemId
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
          var scroll = quickAdd.expanded ? timelineList : dayScroll
          var viewportY = keyCatcher.dragPointerY - scroll.contentY
          if (!quickAdd.expanded) viewportY += flatTimeline.y
          var delta = 0
          if (viewportY < edge) delta = -Math.min(Style.space(8), edge - viewportY)
          else if (viewportY > scroll.height - edge)
            delta = Math.min(Style.space(8), viewportY - (scroll.height - edge))
          var nextY = Math.max(0, Math.min(scroll.contentHeight - scroll.height, scroll.contentY + delta))
          var applied = nextY - scroll.contentY
          if (applied !== 0) {
            scroll.contentY = nextY
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

      Connections {
        target: root
        function onEpicsChanged() { if (root.report) keyCatcher.syncTimeline(root.report) }
      }

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

          DueReminders {
            width: parent.width
            host: root
            fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
          }

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

          RunningCard {
            id: runningCard
            width: parent.width
            visible: root.runningTimer !== null
            host: root
          }

          SessionPanel {
            id: sessionEditor
            visible: root.sessionPanelMode !== ""
            mode: root.sessionPanelMode
            doneItem: root.doneItem
            pauseTitle: root.runningTitle
            pauseSession: root.runningTimer && root.runningTimer.id === root.pauseId ? root.runningTimer : null
            pauseMetrics: root.runningItem && root.runningItem.metrics ? root.runningItem.metrics
              : (root.report && root.report.prompt || [])
            elapsed: root.workedMinutes
            items: root.planner && root.planner.items ? root.planner.items : []
            projects: root.projects
            epics: root.epics
            metricCatalogue: root.metricCatalogue
            busy: root.sessionPanelOp !== ""
            errorText: root.sessionPanelError
            strings: root.tr
            fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
            foreground: root.foreground
            mutedForeground: root.mutedForeground
            onDoneRequested: function(item, values, note) { root.tickDone(item, values, note) }
            onPauseRequested: function(params) { root.savePause(params) }
            onLogRequested: function(target, minutes, values, note, epicId) {
              root.logTime(target, minutes, values, note, epicId)
            }
            onCancelRequested: root.closeSessionPanel()
            onOpenAppRequested: root.openApp()
          }

          QuickAdd {
            id: quickAdd
            transport: client
            onExpandedChanged: Qt.callLater(keyCatcher.revealCreationBlock)
            onHeightChanged: if (expanded) Qt.callLater(keyCatcher.revealCreationBlock)
            strings: root.tr
            host: root
            fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
            foreground: root.foreground
            mutedForeground: root.mutedForeground
            occurrenceHint: keyCatcher.occurrenceHint
            actionError: root.actionError
          }

          Button {
            visible: root.sessionPanelMode !== "log"
            height: Style.space(26)
            text: strings.t("panel.log")
            foreground: root.foreground
            fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
            fontSize: Style.font.bodySmall
            horizontalPadding: Style.space(8)
            verticalPadding: 0
            bordered: true
            onClicked: root.openLogPanel("")
          }

          PanelSeparator {
            visible: root.report !== null
            foreground: root.foreground
          }

          Item {
            id: flatTimeline
            visible: root.report !== null
            width: parent.width
            height: quickAdd.expanded ? Math.min(timelineList.contentHeight,
              Math.max(Style.space(96), keyCatcher.maxContentHeight - (y - quickAdd.y))) : timelineList.contentHeight

            ListView {
              id: timelineList
              anchors.fill: parent
              interactive: quickAdd.expanded && contentHeight > height
              model: timelineModel
              clip: quickAdd.expanded
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
                keyCatcher.dragPointerY - timelineList.contentY - height / 2))
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
