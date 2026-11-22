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

  readonly property string nextName: report && report.next
    ? String(report.next.name || "")
    : ""
  readonly property string nextTime: report && report.next
    ? String(report.next.time || "")
    : ""
  readonly property string remainingLong: formatRemaining(remainingSeconds, false)
  readonly property string remainingShort: formatRemaining(remainingSeconds, true)
  readonly property bool sleeping: report && report.current_block
    ? report.current_block.sleeping === true
    : false
  readonly property string displayText: nextName !== ""
    ? (sleeping
        ? strings.t("bar.sleeping") + "  " + nextName + " " + remainingShort
        : nextName + "  " + remainingShort)
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
      if (hours > 0) return hours + "h " + pad(minutes) + "m"
      if (minutes > 0) return minutes + "m"
      return secs + "s"
    }
    return pad(hours) + ":" + pad(minutes) + ":" + pad(secs)
  }

  function refresh() {
    if (!statusProcess.running) statusProcess.running = true
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
    tooltipText: root.nextName !== ""
      ? root.nextName + " · " + root.nextTime + " · " + root.remainingLong
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
    contentWidth: fittedContentWidth(Style.space(360))
    contentHeight: fittedContentHeight(loadedContentHeight)

    content: PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      implicitHeight: contentColumn.implicitHeight
      onCloseRequested: root.close()
      onTabRequested: function(direction) {
        if (root.bar && typeof root.bar.switchPanelFrom === "function")
          root.bar.switchPanelFrom(root, direction)
      }
      onActivateRequested: root.refresh()

      Column {
        id: contentColumn
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(14)

        Item {
          width: parent.width
          implicitHeight: heroColumn.implicitHeight

          Column {
            id: heroColumn
            anchors.left: parent.left
            anchors.right: heroTime.left
            anchors.rightMargin: Style.space(12)
            spacing: Style.space(3)

            Text {
              width: parent.width
              textFormat: Text.PlainText
              text: root.sleeping
                ? strings.t("panel.sleepingNext")
                : strings.t("panel.nextBoundary")
              color: root.mutedForeground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.caption
              font.bold: true
              font.letterSpacing: 1.2
            }

            Text {
              width: parent.width
              textFormat: Text.PlainText
              text: root.nextName || "—"
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

        Button {
          width: parent.width
          text: strings.t("panel.refreshNow")
          foreground: root.foreground
          fontFamily: root.bar ? root.bar.fontFamily : Style.font.family
          bordered: true
          onClicked: root.syncLocation(true)
        }

        PanelSeparator {
          visible: root.report !== null
          foreground: root.foreground
        }

        Column {
          visible: root.report !== null
          width: parent.width
          spacing: Style.space(2)

          Repeater {
            model: root.report
              ? (root.report.boundaries || root.report.prayers || [])
              : []

            Item {
              required property var modelData
              width: contentColumn.width
              height: Style.space(36)

              Rectangle {
                anchors.fill: parent
                radius: Math.min(Style.cornerRadius, Style.space(6))
                color: modelData.is_next
                  ? Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.10)
                  : "transparent"
              }

              Text {
                anchors.left: parent.left
                anchors.leftMargin: Style.space(10)
                anchors.verticalCenter: parent.verticalCenter
                textFormat: Text.PlainText
                text: String(modelData.name || "")
                color: modelData.passed ? root.mutedForeground : root.foreground
                font.family: root.bar ? root.bar.fontFamily : Style.font.family
                font.pixelSize: Style.font.body
                font.bold: modelData.is_next === true
              }

              Text {
                anchors.right: parent.right
                anchors.rightMargin: Style.space(10)
                anchors.verticalCenter: parent.verticalCenter
                textFormat: Text.PlainText
                text: String(modelData.time || "—")
                color: modelData.passed ? root.mutedForeground : root.foreground
                font.family: root.bar ? root.bar.fontFamily : Style.font.family
                font.pixelSize: Style.font.body
                font.bold: modelData.is_next === true
              }
            }
          }
        }

        Column {
          visible: root.report === null
          width: parent.width
          spacing: Style.space(6)

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
          width: parent.width
          textFormat: Text.PlainText
          text: strings.t("panel.usingSavedLocation").arg(root.locationWarning)
          color: root.mutedForeground
          font.family: root.bar ? root.bar.fontFamily : Style.font.family
          font.pixelSize: Style.font.caption
          wrapMode: Text.WrapAnywhere
          maximumLineCount: 3
          elide: Text.ElideRight
        }

        Text {
          visible: root.officialWarning !== "" && root.report && root.report.source !== "official"
          width: parent.width
          textFormat: Text.PlainText
          text: strings.t("panel.usingCalculatedTimes").arg(root.officialWarning)
          color: root.mutedForeground
          font.family: root.bar ? root.bar.fontFamily : Style.font.family
          font.pixelSize: Style.font.caption
          wrapMode: Text.WrapAnywhere
          maximumLineCount: 3
          elide: Text.ElideRight
        }

        Item {
          width: parent.width
          implicitHeight: Math.max(locationLabel.implicitHeight, methodLabel.implicitHeight)

          Text {
            id: locationLabel
            anchors.left: parent.left
            anchors.right: methodLabel.left
            anchors.rightMargin: Style.space(10)
            textFormat: Text.PlainText
            text: root.report
              ? (root.autoLocation
                  ? strings.t("panel.autoLocation").arg(String(root.report.location || ""))
                  : String(root.report.location || ""))
              : ""
            color: root.mutedForeground
            font.family: root.bar ? root.bar.fontFamily : Style.font.family
            font.pixelSize: Style.font.caption
            elide: Text.ElideRight
          }

          Text {
            id: methodLabel
            anchors.right: parent.right
            textFormat: Text.PlainText
            text: root.report ? String(root.report.source_name || root.report.method_name || root.report.method || "") : ""
            color: root.mutedForeground
            font.family: root.bar ? root.bar.fontFamily : Style.font.family
            font.pixelSize: Style.font.caption
            elide: Text.ElideRight
          }
        }

        Text {
          visible: root.autoLocation && root.report !== null
          width: parent.width
          textFormat: Text.PlainText
          text: strings.t("panel.networkLocationNotice")
          color: root.mutedForeground
          font.family: root.bar ? root.bar.fontFamily : Style.font.family
          font.pixelSize: Style.font.caption
          wrapMode: Text.WordWrap
        }
      }
    }
  }
}
