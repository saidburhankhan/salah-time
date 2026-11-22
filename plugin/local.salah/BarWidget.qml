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
  readonly property bool sleeping: report && report.current_block
    ? report.current_block.sleeping === true
    : false
  readonly property var planner: report && report.planner ? report.planner : null
  readonly property int openCount: planner ? (Number(planner.open) || 0) : 0
  readonly property string displayText: localizedNextName !== ""
    ? (sleeping
        ? strings.t("bar.sleeping") + "  " + localizedNextName + " " + remainingShort
        : localizedNextName + "  " + remainingShort + (openCount > 0 ? "  · " + openCount : ""))
    : (firstLoad ? strings.t("bar.loading") : "☾ —")
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
    return pad(hours) + ":" + pad(minutes) + ":" + pad(secs)
  }

  function refresh() {
    if (!statusProcess.running) statusProcess.running = true
  }

  // Planner actions run one at a time, in order, as fixed argument lists.
  // Titles follow `--` so text starting with "-" is never read as a flag.
  function runAction(args) {
    pendingActions = pendingActions.concat([["salah"].concat(args)])
    runNextAction()
  }

  function runNextAction() {
    if (actionProcess.running || pendingActions.length === 0) return
    actionProcess.command = pendingActions[0]
    pendingActions = pendingActions.slice(1)
    actionProcess.running = true
  }

  function addItem(title) {
    var text = String(title || "").trim()
    if (text === "") return
    var args = ["item", "add", "--json", "--quick"]
    if (quickAddBlock !== "") args = args.concat(["--block", quickAddBlock])
    runAction(args.concat(["--", text]))
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
    var key = "prayer." + String(id || "")
    var translated = strings.t(key)
    return translated === key ? String(fallback || id || "") : translated
  }

  function isPrayerMarked(id) {
    var marks = planner && planner.prayer_marks ? planner.prayer_marks : []
    return marks.indexOf(String(id || "")) >= 0
  }

  function togglePrayer(prayer) {
    var command = isPrayerMarked(prayer.id) ? "unmark" : "mark"
    runAction(["prayer", command, "--json", String(prayer.id)])
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
    runAction(["item", row.state === "done" ? "undo" : "done", "--json", String(row.id)])
  }

  function toggleTimer(row) {
    if (row.timer_started_at) runAction(["item", "stop", "--json"])
    else runAction(["item", "start", "--json", String(row.id)])
  }

  function logOne(row) {
    runAction(["item", "log", "--json", String(row.id), "1"])
  }

  function setSleepTime(value) {
    var text = String(value || "").trim()
    if (!/^([01][0-9]|2[0-3]):[0-5][0-9]$/.test(text)) {
      actionError = strings.t("panel.sleepFormat")
      return
    }
    runAction(["config", "set", "--sleep-time", text])
  }

  function itemDetail(row) {
    var parts = []
    if (row.measure === "minutes")
      parts.push(row.minutes + (row.target ? "/" + row.target : "") + " " + strings.t("panel.minutesShort"))
    else if (row.measure === "count")
      parts.push(row.count + (row.target ? "/" + row.target : ""))
    if (row.repeat === "daily") parts.push(strings.t("repeat.daily"))
    else if (row.repeat === "weekly") parts.push(strings.t("repeat.weekly"))
    return parts.join(" · ")
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

  function close() { panel.open = false }
  function toggle() { opened ? close() : open() }
  function closeForPopoutSwitch() {
    popoutSwitchClosing = true
    close()
    Qt.callLater(function() { root.popoutSwitchClosing = false })
  }

  function parseReport(raw) {
    try {
      var parsed = JSON.parse(String(raw || "").trim())
      if (!parsed || !parsed.next || !(parsed.prayers instanceof Array))
        throw new Error("missing next/prayers fields")
      report = parsed
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
    stderr: StdioCollector {
      id: actionStderr
      waitForEnd: true
    }
    onExited: function(exitCode) {
      if (exitCode === 0) root.actionError = ""
      else {
        var detail = String(actionStderr.text || "").trim().replace(/[\r\n]+/g, " ")
        root.actionError = detail === "" ? strings.t("panel.commandFailed") : detail.slice(0, 240)
      }
      root.refresh()
      root.runNextAction()
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

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.vertical ? "☾" : root.displayText
    labelVisible: true
    horizontalMargin: 8.75
    verticalPadding: 8.75
    tooltipText: root.localizedNextName !== ""
      ? root.localizedNextName + " · " + root.nextTime + " · " + root.remainingLong
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
    contentWidth: fittedContentWidth(Style.space(400))
    contentHeight: fittedContentHeight(loadedContentHeight)

    content: PanelKeyCatcher {
      id: keyCatcher
      property alias quickField: quickField
      anchors.fill: parent
      implicitHeight: Math.min(dayColumn.implicitHeight,
        Math.max(Style.space(240), panel.availableCardHeight - panel.padding * 2))
      blocked: quickField.activeFocus || sleepField.activeFocus
      onCloseRequested: root.close()
      onTabRequested: function(direction) {
        if (root.bar && typeof root.bar.switchPanelFrom === "function")
          root.bar.switchPanelFrom(root, direction)
      }
      onActivateRequested: root.refresh()

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
                text: root.sleeping
                  ? strings.t("panel.sleepingNext")
                  : strings.t("panel.next")
                foreground: root.foreground
                fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
              }

              Text {
                width: parent.width
                textFormat: Text.PlainText
                text: root.prayerName(root.report && root.report.next ? root.report.next.id : "", root.nextName) || "—"
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
              text: root.nextTime || "—"
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

          Column {
            id: timeline
            visible: root.report !== null
            width: parent.width
            spacing: 0

            Repeater {
              model: root.report ? (root.report.prayers || []).filter(function(row) {
                return ["fajr", "dhuhr", "asr", "maghrib", "isha"].indexOf(String(row.id || "")) >= 0
              }) : []

              Column {
                id: prayerBlock
                required property var modelData
                readonly property bool current: !root.sleeping && root.report
                  && root.report.current_block
                  && String(root.report.current_block.start.id || "") === String(modelData.id || "")
                readonly property bool marked: root.isPrayerMarked(modelData.id)
                readonly property var blockRows: root.itemsForBlock(modelData.id)
                width: timeline.width
                spacing: Style.space(2)
                opacity: root.sleeping || (modelData.passed && !current) ? 0.58 : 1

                Rectangle {
                  width: parent.width
                  height: Style.space(40)
                  radius: Style.cornerRadius
                  color: prayerBlock.current
                    ? Style.selectedFillFor(root.foreground, Commons.Color.accent)
                    : (prayerHover.containsMouse
                        ? Style.hoverFillFor(root.foreground, Commons.Color.accent)
                        : "transparent")

                  Rectangle {
                    id: prayerCircle
                    anchors.left: parent.left
                    anchors.leftMargin: Style.space(8)
                    anchors.verticalCenter: parent.verticalCenter
                    width: Style.space(18)
                    height: width
                    radius: width / 2
                    color: prayerBlock.marked
                      ? Style.selectedStateColor(root.foreground, Commons.Color.accent)
                      : "transparent"
                    border.width: Style.spacing.hairline
                    border.color: prayerBlock.marked
                      ? Style.selectedStateColor(root.foreground, Commons.Color.accent)
                      : root.foreground

                    Text {
                      visible: prayerBlock.marked
                      anchors.centerIn: parent
                      textFormat: Text.PlainText
                      text: "✓"
                      color: Commons.Color.background
                      font.family: root.bar ? root.bar.fontFamily : Style.font.family
                      font.pixelSize: Style.font.caption
                      font.bold: true
                    }

                    MouseArea {
                      anchors.fill: parent
                      anchors.margins: -Style.space(6)
                      cursorShape: Qt.PointingHandCursor
                      onClicked: root.togglePrayer(prayerBlock.modelData)
                    }
                  }

                  MouseArea {
                    id: prayerHover
                    anchors.left: prayerCircle.right
                    anchors.right: addBlockButton.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    hoverEnabled: true
                    acceptedButtons: Qt.NoButton
                  }

                  Text {
                    anchors.left: prayerCircle.right
                    anchors.leftMargin: Style.space(10)
                    anchors.verticalCenter: parent.verticalCenter
                    textFormat: Text.PlainText
                    text: root.prayerName(prayerBlock.modelData.id, prayerBlock.modelData.name)
                    color: root.foreground
                    font.family: root.bar ? root.bar.fontFamily : Style.font.family
                    font.pixelSize: Style.font.body
                    font.bold: prayerBlock.current
                  }

                  Text {
                    anchors.right: nowTag.left
                    anchors.rightMargin: Style.space(8)
                    anchors.verticalCenter: parent.verticalCenter
                    textFormat: Text.PlainText
                    text: String(prayerBlock.modelData.time || "—")
                    color: root.mutedForeground
                    font.family: root.bar ? root.bar.fontFamily : Style.font.family
                    font.pixelSize: Style.font.bodySmall
                  }

                  Rectangle {
                    id: nowTag
                    visible: prayerBlock.current
                    anchors.right: addBlockButton.left
                    anchors.rightMargin: Style.space(5)
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

                  Button {
                    id: addBlockButton
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    width: Style.space(34)
                    height: Style.space(30)
                    text: strings.t("panel.addSymbol")
                    tooltipText: strings.t("panel.addTo").arg(root.prayerName(prayerBlock.modelData.id, prayerBlock.modelData.name))
                    foreground: root.foreground
                    fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
                    horizontalPadding: Style.space(6)
                    verticalPadding: Style.space(2)
                    onClicked: root.presetQuickAdd(prayerBlock.modelData.id)
                  }
                }

                Repeater {
                  model: prayerBlock.blockRows

                  Item {
                    id: itemRow
                    required property var modelData
                    readonly property bool done: modelData.state === "done"
                    width: prayerBlock.width
                    height: Style.space(38)

                    Rectangle {
                      anchors.fill: parent
                      anchors.leftMargin: Style.space(28)
                      radius: Style.cornerRadius
                      color: itemMouse.containsMouse
                        ? Style.hoverFillFor(root.foreground, Commons.Color.accent)
                        : "transparent"
                    }

                    Rectangle {
                      id: itemCheck
                      anchors.left: parent.left
                      anchors.leftMargin: Style.space(38)
                      anchors.verticalCenter: parent.verticalCenter
                      width: Style.space(15)
                      height: width
                      radius: Math.min(Style.cornerRadius, Style.space(3))
                      color: itemRow.done
                        ? Style.selectedStateColor(root.foreground, Commons.Color.accent)
                        : "transparent"
                      border.width: Style.spacing.hairline
                      border.color: itemRow.done
                        ? Style.selectedStateColor(root.foreground, Commons.Color.accent)
                        : root.mutedForeground

                      Text {
                        visible: itemRow.done
                        anchors.centerIn: parent
                        textFormat: Text.PlainText
                        text: "✓"
                        color: Commons.Color.background
                        font.family: root.bar ? root.bar.fontFamily : Style.font.family
                        font.pixelSize: Style.font.caption
                      }
                    }

                    MouseArea {
                      id: itemMouse
                      anchors.left: parent.left
                      anchors.right: itemAction.left
                      anchors.top: parent.top
                      anchors.bottom: parent.bottom
                      hoverEnabled: true
                      cursorShape: Qt.PointingHandCursor
                      onClicked: root.toggleItem(itemRow.modelData)
                    }

                    Column {
                      anchors.left: itemCheck.right
                      anchors.leftMargin: Style.space(9)
                      anchors.right: itemAction.left
                      anchors.rightMargin: Style.space(6)
                      anchors.verticalCenter: parent.verticalCenter
                      spacing: 0

                      Text {
                        width: parent.width
                        textFormat: Text.PlainText
                        text: String(itemRow.modelData.title || "")
                        color: itemRow.done ? root.mutedForeground : root.foreground
                        font.family: root.bar ? root.bar.fontFamily : Style.font.family
                        font.pixelSize: Style.font.bodySmall
                        font.strikeout: itemRow.done
                        elide: Text.ElideRight
                      }

                      Text {
                        visible: text !== ""
                        width: parent.width
                        textFormat: Text.PlainText
                        text: root.itemDetail(itemRow.modelData)
                        color: root.mutedForeground
                        font.family: root.bar ? root.bar.fontFamily : Style.font.family
                        font.pixelSize: Style.font.caption
                        elide: Text.ElideRight
                      }
                    }

                    Button {
                      id: itemAction
                      visible: itemRow.modelData.measure !== "check"
                      anchors.right: parent.right
                      anchors.rightMargin: Style.space(3)
                      anchors.verticalCenter: parent.verticalCenter
                      width: visible ? Style.space(40) : 0
                      height: Style.space(28)
                      text: itemRow.modelData.measure === "count"
                        ? strings.t("panel.logOne")
                        : (itemRow.modelData.timer_started_at ? strings.t("panel.stop") : strings.t("panel.start"))
                      tooltipText: itemRow.modelData.measure === "count"
                        ? strings.t("panel.logOneHint")
                        : (itemRow.modelData.timer_started_at ? strings.t("panel.stopTimer") : strings.t("panel.startTimer"))
                      foreground: root.foreground
                      fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
                      horizontalPadding: Style.space(5)
                      verticalPadding: Style.space(2)
                      bordered: true
                      onClicked: itemRow.modelData.measure === "count"
                        ? root.logOne(itemRow.modelData)
                        : root.toggleTimer(itemRow.modelData)
                    }
                  }
                }

                Item {
                  visible: String(prayerBlock.modelData.id || "") === "fajr" && root.boundary("sunrise") !== null
                  width: parent.width
                  height: visible ? Style.space(24) : 0

                  Text {
                    anchors.left: parent.left
                    anchors.leftMargin: Style.space(38)
                    anchors.verticalCenter: parent.verticalCenter
                    textFormat: Text.PlainText
                    text: strings.t("boundary.sunrise")
                    color: root.mutedForeground
                    font.family: root.bar ? root.bar.fontFamily : Style.font.family
                    font.pixelSize: Style.font.caption
                  }

                  Text {
                    anchors.right: parent.right
                    anchors.rightMargin: Style.space(10)
                    anchors.verticalCenter: parent.verticalCenter
                    textFormat: Text.PlainText
                    text: root.boundary("sunrise") ? String(root.boundary("sunrise").time || "—") : ""
                    color: root.mutedForeground
                    font.family: root.bar ? root.bar.fontFamily : Style.font.family
                    font.pixelSize: Style.font.caption
                  }
                }

                Item {
                  visible: String(prayerBlock.modelData.id || "") === "isha" && root.boundary("sleep") !== null
                  width: parent.width
                  height: visible ? Style.space(24) : 0

                  Text {
                    anchors.left: parent.left
                    anchors.leftMargin: Style.space(38)
                    anchors.verticalCenter: parent.verticalCenter
                    textFormat: Text.PlainText
                    text: strings.t("boundary.sleep")
                    color: root.mutedForeground
                    font.family: root.bar ? root.bar.fontFamily : Style.font.family
                    font.pixelSize: Style.font.caption
                  }

                  Text {
                    anchors.right: parent.right
                    anchors.rightMargin: Style.space(10)
                    anchors.verticalCenter: parent.verticalCenter
                    textFormat: Text.PlainText
                    text: root.boundary("sleep") ? String(root.boundary("sleep").time || "—") : ""
                    color: root.mutedForeground
                    font.family: root.bar ? root.bar.fontFamily : Style.font.family
                    font.pixelSize: Style.font.caption
                  }
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
          }

          Column {
            visible: root.sleeping && root.planner !== null
            width: parent.width
            spacing: Style.space(3)

            PanelSeparator {
              foreground: root.foreground
            }

            PanelSectionHeader {
              text: strings.t("panel.tomorrow")
              foreground: root.foreground
              fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
            }

            Repeater {
              model: root.planner ? (root.planner.upcoming || []) : []

              Item {
                id: upcomingRow
                required property var modelData
                width: parent.width
                height: Style.space(34)

                Text {
                  anchors.left: parent.left
                  anchors.leftMargin: Style.space(8)
                  anchors.verticalCenter: parent.verticalCenter
                  width: Style.space(68)
                  textFormat: Text.PlainText
                  text: root.prayerName(upcomingRow.modelData.block, upcomingRow.modelData.block)
                  color: root.mutedForeground
                  font.family: root.bar ? root.bar.fontFamily : Style.font.family
                  font.pixelSize: Style.font.caption
                  elide: Text.ElideRight
                }

                Text {
                  anchors.left: parent.left
                  anchors.leftMargin: Style.space(84)
                  anchors.right: parent.right
                  anchors.verticalCenter: parent.verticalCenter
                  textFormat: Text.PlainText
                  text: String(upcomingRow.modelData.title || "")
                  color: root.foreground
                  font.family: root.bar ? root.bar.fontFamily : Style.font.family
                  font.pixelSize: Style.font.bodySmall
                  elide: Text.ElideRight
                }
              }
            }
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

          Item {
            visible: root.report !== null
            width: parent.width
            implicitHeight: sleepField.implicitHeight

            Text {
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter
              textFormat: Text.PlainText
              text: strings.t("panel.sleepTime")
              color: root.mutedForeground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.caption
            }

            TextField {
              id: sleepField
              anchors.right: parent.right
              width: Style.space(72)
              horizontalAlignment: TextInput.AlignHCenter
              maximumLength: 5
              inputMethodHints: Qt.ImhDigitsOnly
              foreground: root.foreground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family

              function showSaved() {
                if (!activeFocus) text = String(root.report ? root.report.sleep_time || "" : "")
              }

              Component.onCompleted: showSaved()
              onActiveFocusChanged: showSaved()
              Connections {
                target: root
                function onReportChanged() { sleepField.showSaved() }
              }
              onAccepted: {
                root.setSleepTime(text)
                focus = false
              }
            }
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
