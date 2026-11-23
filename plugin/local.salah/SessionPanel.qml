import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "SessionLogic.js" as SessionLogic

// Inline editor for pausing a timer, ticking an item or logging minutes.
Rectangle {
  id: panel

  property var strings
  property string mode: "log"
  // done mode: the item being ticked (a planner row: id, title, project).
  property var doneItem: null
  property var pauseSession: null
  property var pauseMetrics: []
  property string pauseTitle: ""
  property int elapsed: 1
  property int initialMinutes: 1
  property var items: []
  property var projects: []
  property var metricCatalogue: []
  property bool busy: false
  property string errorText: ""
  property string fontFamily
  property color foreground
  property color mutedForeground

  property string targetKey: ""
  property var values: ({})
  property string minutesText: ""
  property string hintText: ""

  readonly property bool logMode: mode === "log"
  readonly property bool pauseMode: mode === "pause"
  readonly property var targets: SessionLogic.logTargets(items, projects)
  readonly property var itemTargets: targets.filter(function(t) { return t.kind === "item" })
  readonly property var projectTargets: targets.filter(function(t) { return t.kind === "project" })
  readonly property var target: {
    if (!logMode) {
      if (!doneItem) return null
      return {
        "kind": "item",
        "id": String(doneItem.id || ""),
        "label": String(doneItem.title || ""),
        "metrics": doneItem.metrics,
        "projectId": doneItem.project ? String(doneItem.project.id || "") : ""
      }
    }
    for (var i = 0; i < targets.length; i++)
      if (targets[i].kind + ":" + targets[i].id === targetKey) return targets[i]
    return null
  }
  readonly property var askedMetrics: pauseMode ? pauseMetrics
    : SessionLogic.targetMetrics(target, projects, metricCatalogue)
  // A copy that only changes when the asked metrics really do, so a status
  // refresh does not rebuild the fields (and drop a half-typed count).
  property var metrics: []
  onAskedMetricsChanged: {
    if (JSON.stringify(askedMetrics) !== JSON.stringify(metrics)) metrics = askedMetrics
  }
  Component.onCompleted: metrics = askedMetrics
  readonly property var minutes: SessionLogic.parseMinutes(minutesText)
  onElapsedChanged: {
    if (pauseMode && minutesField && !minutesField.activeFocus && minutes === initialMinutes) {
      initialMinutes = elapsed
      minutesText = String(elapsed)
      minutesField.text = minutesText
    }
  }
  // A metric's count field reports its focus here, so the panel's key
  // catcher can step aside while text is typed.
  property bool metricFocus: false
  readonly property bool textFocus: minutesField.activeFocus || noteField.activeFocus || metricFocus
  // A field destroyed while focused never reports losing focus.
  onVisibleChanged: if (!visible) metricFocus = false

  signal doneRequested(var item, var values, string note)
  signal pauseRequested(var params)
  signal logRequested(var target, int minutes, var values, string note)
  signal cancelRequested()
  signal openAppRequested()

  // Opens the panel fresh. `presetKey` ("item:<id>" / "project:<id>")
  // preselects a log target, e.g. from an item's "+ Time".
  function reset(presetKey) {
    metricFocus = false
    values = ({})
    hintText = ""
    initialMinutes = elapsed
    minutesText = pauseMode ? String(initialMinutes) : ""
    minutesField.text = minutesText
    noteField.text = ""
    targetKey = String(presetKey || "")
    if (targetKey === "" && targets.length === 1) targetKey = targets[0].kind + ":" + targets[0].id
    resetMetricFields()
  }

  function resetMetricFields() {
    for (var i = 0; i < metricRepeater.count; i++) {
      var child = metricRepeater.itemAt(i)
      if (child) child.reset()
    }
  }

  function setValue(id, value) {
    var copy = {}
    for (var key in values) copy[key] = values[key]
    if (value === null || value === undefined) delete copy[String(id)]
    else copy[String(id)] = value
    values = copy
  }

  function selectTarget(key) {
    if (targetKey === key) return
    targetKey = key
    values = ({})
    hintText = ""
    resetMetricFields()
  }

  function savePause(done) {
    if (busy || !pauseSession) return
    if (minutesText.trim() && (minutes === null || minutes > elapsed)) {
      hintText = strings.t("session.workedRange").arg(elapsed)
      minutesField.forceActiveFocus()
      return
    }
    hintText = ""
    pauseRequested(SessionLogic.pauseParams(pauseSession, metrics, values, noteField.text,
      minutesText, initialMinutes, done))
  }

  // Done: tick with whatever is filled. Skip: tick with nothing.
  function save(skip) {
    if (busy) return
    if (pauseMode) {
      savePause(false)
      return
    }
    if (!logMode) {
      if (!doneItem) return
      if (skip === true) doneRequested(doneItem, {}, "")
      else doneRequested(doneItem, SessionLogic.collectValues(metrics, values), noteField.text.trim())
      return
    }
    // A log needs something to log against and how long; say so rather
    // than disabling Save.
    if (target === null) {
      hintText = strings.t("session.needTarget")
      return
    }
    if (minutes === null) {
      hintText = strings.t("session.needMinutes")
      minutesField.forceActiveFocus()
      return
    }
    hintText = ""
    logRequested(target, minutes, SessionLogic.collectValues(metrics, values), noteField.text.trim())
  }

  width: parent ? parent.width : 0
  implicitHeight: form.implicitHeight + Style.space(20)
  radius: Style.cornerRadius
  color: Style.hoverFillFor(foreground, Commons.Color.accent)
  border.width: Style.spacing.hairline
  border.color: Style.normalBorderFor(foreground, Commons.Color.accent)

  Column {
    id: form
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.margins: Style.space(10)
    spacing: Style.space(8)

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: panel.pauseMode ? panel.strings.t("session.pauseTitle").arg(panel.pauseTitle)
        : panel.logMode ? panel.strings.t("session.logTitle")
        : panel.strings.t("session.doneTitle").arg(panel.target ? panel.target.label : "")
      color: panel.foreground
      font.family: panel.fontFamily
      font.pixelSize: Style.font.body
      font.bold: true
      elide: Text.ElideRight
    }

    // ---- log target -------------------------------------------------------

    Column {
      visible: panel.logMode && panel.targets.length === 0
      width: parent.width
      spacing: Style.space(6)

      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: panel.strings.t("session.nothingToLog")
        color: panel.mutedForeground
        font.family: panel.fontFamily
        font.pixelSize: Style.font.bodySmall
        wrapMode: Text.WordWrap
      }

      Button {
        text: panel.strings.t("session.openApp")
        bordered: true
        foreground: panel.foreground
        fontFamily: panel.fontFamily
        fontSize: Style.font.bodySmall
        onClicked: panel.openAppRequested()
      }
    }

    Repeater {
      model: panel.logMode ? [
        { "caption": panel.strings.t("session.today"), "rows": panel.itemTargets },
        { "caption": panel.strings.t("session.projects"), "rows": panel.projectTargets }
      ] : []

      Column {
        required property var modelData
        visible: modelData.rows.length > 0
        width: form.width
        spacing: Style.space(3)

        Text {
          textFormat: Text.PlainText
          text: modelData.caption
          color: panel.mutedForeground
          font.family: panel.fontFamily
          font.pixelSize: Style.font.caption
        }

        Flow {
          width: parent.width
          spacing: Style.space(4)

          Repeater {
            model: modelData.rows

            Button {
              required property var modelData
              readonly property string key: modelData.kind + ":" + modelData.id
              text: modelData.label
              // An em space keeps room for the project colour dot.
              iconText: modelData.color !== "" ? " " : ""
              iconSize: Style.font.caption
              leftAlign: true
              selected: panel.targetKey === key
              bordered: true
              foreground: panel.foreground
              fontFamily: panel.fontFamily
              fontSize: Style.font.bodySmall
              horizontalPadding: Style.space(6)
              verticalPadding: Style.space(2)
              width: Math.min(implicitWidth, form.width)
              onClicked: panel.selectTarget(key)

              Rectangle {
                visible: modelData.color !== ""
                x: parent._reservedContentLeftInset
                anchors.verticalCenter: parent.verticalCenter
                width: Style.space(7)
                height: width
                radius: width / 2
                color: modelData.color !== "" ? modelData.color : "transparent"
              }
            }
          }
        }
      }
    }

    // ---- minutes ----------------------------------------------------------

    Row {
      visible: panel.pauseMode || (panel.logMode && panel.targets.length > 0)
      spacing: Style.space(4)

      Text {
        anchors.verticalCenter: parent.verticalCenter
        width: Style.space(96)
        textFormat: Text.PlainText
        text: panel.pauseMode ? panel.strings.t("session.worked") : panel.strings.t("session.minutes")
        color: panel.foreground
        font.family: panel.fontFamily
        font.pixelSize: Style.font.bodySmall
      }

      TextField {
        id: minutesField
        width: Style.space(62)
        height: Style.space(26)
        placeholderText: panel.strings.t("session.minutesUnit")
        foreground: panel.foreground
        font.family: panel.fontFamily
        font.pixelSize: Style.font.bodySmall
        horizontalPadding: Style.space(5)
        verticalPadding: 0
        maximumLength: 4
        inputMethodHints: Qt.ImhDigitsOnly
        onTextEdited: panel.minutesText = text
        onAccepted: panel.save(false)
        Keys.onEscapePressed: function(event) {
          panel.cancelRequested()
          event.accepted = true
        }
      }

      Repeater {
        model: panel.pauseMode ? [5, 15, 30] : [15, 30, 60]

        Button {
          required property int modelData
          width: Style.space(34)
          height: Style.space(24)
          text: panel.pauseMode ? "−" + modelData : String(modelData)
          selected: !panel.pauseMode && panel.minutes === modelData
          bordered: true
          foreground: panel.foreground
          fontFamily: panel.fontFamily
          fontSize: Style.font.bodySmall
          horizontalPadding: 0
          verticalPadding: 0
          onClicked: {
            var next = panel.pauseMode
              ? Math.max(1, (panel.minutes || panel.initialMinutes) - modelData) : modelData
            minutesField.text = String(next)
            panel.minutesText = minutesField.text
          }
        }
      }
    }

    // ---- metrics (all optional) -----------------------------------------

    Repeater {
      id: metricRepeater
      model: panel.logMode && panel.targets.length === 0 ? [] : panel.metrics

      MetricField {
        required property var modelData
        width: form.width
        strings: panel.strings
        metric: modelData
        storedValue: panel.pauseMode && panel.pauseSession && panel.pauseSession.values
          ? panel.pauseSession.values[metricId] : undefined
        value: panel.values[metricId] !== undefined ? panel.values[metricId] : storedValue
        fontFamily: panel.fontFamily
        foreground: panel.foreground
        mutedForeground: panel.mutedForeground
        onValueEdited: function(value) { panel.setValue(modelData.metric_id, value) }
        onEscapePressed: panel.cancelRequested()
        onTextFocusChanged: panel.metricFocus = textFocus
      }
    }

    TextField {
      id: noteField
      visible: !panel.logMode || panel.targets.length > 0
      width: parent.width
      height: Style.space(28)
      placeholderText: panel.strings.t("session.notePlaceholder")
      foreground: panel.foreground
      font.family: panel.fontFamily
      font.pixelSize: Style.font.bodySmall
      verticalPadding: 0
      maximumLength: 240
      onAccepted: panel.save(false)
      Keys.onEscapePressed: function(event) {
        panel.cancelRequested()
        event.accepted = true
      }
    }

    Text {
      visible: text !== ""
      width: parent.width
      textFormat: Text.PlainText
      text: panel.errorText !== "" ? panel.errorText : panel.hintText
      color: panel.errorText !== "" ? Commons.Color.urgent : panel.mutedForeground
      font.family: panel.fontFamily
      font.pixelSize: Style.font.caption
      wrapMode: Text.Wrap
      maximumLineCount: 3
      elide: Text.ElideRight
    }

    Row {
      anchors.right: parent.right
      spacing: Style.space(6)

      Button {
        text: panel.strings.t("session.cancel")
        foreground: panel.mutedForeground
        fontFamily: panel.fontFamily
        fontSize: Style.font.bodySmall
        onClicked: panel.cancelRequested()
      }

      Button {
        visible: panel.mode === "done"
        enabled: !panel.busy
        text: panel.strings.t("session.skip")
        tooltipText: panel.strings.t("session.skipTooltip")
        bordered: true
        foreground: panel.mutedForeground
        fontFamily: panel.fontFamily
        fontSize: Style.font.bodySmall
        onClicked: panel.save(true)
      }

      Button {
        visible: !panel.logMode || panel.targets.length > 0
        enabled: !panel.busy
        text: panel.busy ? panel.strings.t("session.saving")
          : (panel.mode === "done" ? panel.strings.t("session.done") : panel.strings.t("session.save"))
        bordered: true
        foreground: panel.foreground
        fontFamily: panel.fontFamily
        fontSize: Style.font.bodySmall
        onClicked: panel.save(false)
      }

      Button {
        visible: panel.pauseMode && !!(panel.pauseSession && panel.pauseSession.item_id)
        enabled: !panel.busy
        text: panel.strings.t("session.saveDone")
        bordered: true
        foreground: panel.foreground
        fontFamily: panel.fontFamily
        fontSize: Style.font.bodySmall
        onClicked: panel.savePause(true)
      }
    }
  }
}
