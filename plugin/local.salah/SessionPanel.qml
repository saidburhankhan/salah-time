import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "SessionLogic.js" as SessionLogic

// Inline panel for finishing work. In "stop" mode it asks the running
// session's metrics and sends session.stop; in "log" mode it also asks what
// the time was for (a today item or a project) and how many minutes, then
// sends session.log. Required metrics come first and must be filled; the
// optional ones may stay empty. The host decides the stop metrics and passes
// the log catalogue (items, projects, metric list) in.
Rectangle {
  id: panel

  property var strings
  property string mode: "stop"
  property string sessionTitle: ""
  property string elapsedText: ""
  property var stopMetrics: []
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
  property var missing: []
  property string minutesText: ""

  readonly property bool logMode: mode === "log"
  readonly property var targets: SessionLogic.logTargets(items, projects)
  readonly property var itemTargets: targets.filter(function(t) { return t.kind === "item" })
  readonly property var projectTargets: targets.filter(function(t) { return t.kind === "project" })
  readonly property var target: {
    for (var i = 0; i < targets.length; i++)
      if (targets[i].kind + ":" + targets[i].id === targetKey) return targets[i]
    return null
  }
  readonly property var askedMetrics: logMode
    ? SessionLogic.logMetrics(target, projects, metricCatalogue)
    : SessionLogic.orderMetrics(stopMetrics)
  // A copy that only changes when the asked metrics really do, so a status
  // refresh does not rebuild the fields (and drop a half-typed count).
  property var metrics: []
  onAskedMetricsChanged: {
    if (JSON.stringify(askedMetrics) !== JSON.stringify(metrics)) metrics = askedMetrics
  }
  Component.onCompleted: metrics = askedMetrics
  // A field destroyed while focused never reports losing focus.
  onVisibleChanged: if (!visible) metricFocus = false
  readonly property var requiredMetrics: metrics.filter(function(m) { return m.required === true })
  readonly property var optionalMetrics: metrics.filter(function(m) { return m.required !== true })
  readonly property var minutes: SessionLogic.parseMinutes(minutesText)
  readonly property var unfilled: SessionLogic.missingRequired(metrics, values)
  readonly property string blocker: {
    if (logMode && targets.length === 0) return " "
    if (logMode && target === null) return strings.t("session.needTarget")
    if (logMode && minutes === null) return strings.t("session.needMinutes")
    if (unfilled.length > 0) return strings.t("session.needMetrics").arg(metricNames(unfilled))
    return ""
  }
  // A metric's count field reports its focus here, so the panel's key
  // catcher can step aside while text is typed.
  property bool metricFocus: false
  readonly property bool textFocus: minutesField.activeFocus || noteField.activeFocus || metricFocus

  signal stopRequested(var values, string note)
  signal logRequested(var target, int minutes, var values, string note)
  signal cancelRequested()
  signal openAppRequested()

  // Opens the panel fresh. `presetKey` ("item:<id>" / "project:<id>")
  // preselects a log target, e.g. from an item's "+ Time".
  function reset(presetKey) {
    metricFocus = false
    values = ({})
    missing = []
    minutesText = ""
    minutesField.text = ""
    noteField.text = ""
    targetKey = String(presetKey || "")
    if (targetKey === "" && targets.length === 1) targetKey = targets[0].kind + ":" + targets[0].id
    resetFields(requiredRepeater)
    resetFields(optionalRepeater)
  }

  function resetFields(repeater) {
    for (var i = 0; i < repeater.count; i++) {
      var child = repeater.itemAt(i)
      if (child) child.reset()
    }
  }

  function metricNames(ids) {
    var names = []
    for (var i = 0; i < ids.length; i++) {
      var name = ids[i]
      for (var m = 0; m < metrics.length; m++)
        if (metrics[m].metric_id === ids[i]) name = metrics[m].name
      names.push(name)
    }
    return names.join(", ")
  }

  function markMissing(ids) {
    missing = (ids || []).map(function(id) { return String(id) })
  }

  function setValue(id, value) {
    var copy = {}
    for (var key in values) copy[key] = values[key]
    if (value === null || value === undefined) delete copy[String(id)]
    else copy[String(id)] = value
    values = copy
    missing = missing.filter(function(key) { return key !== String(id) })
  }

  function selectTarget(key) {
    if (targetKey === key) return
    targetKey = key
    values = ({})
    missing = []
  }

  function save() {
    if (busy || blocker !== "") return
    var sent = SessionLogic.collectValues(metrics, values)
    var note = noteField.text.trim()
    if (logMode) logRequested(target, minutes, sent, note)
    else stopRequested(sent, note)
  }

  width: parent ? parent.width : 0
  implicitHeight: form.implicitHeight + Style.space(20)
  radius: Style.cornerRadius
  color: Style.hoverFillFor(foreground, Commons.Color.accent)

  Column {
    id: form
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.margins: Style.space(10)
    spacing: Style.space(8)

    Item {
      width: parent.width
      implicitHeight: heading.implicitHeight

      Text {
        id: heading
        anchors.left: parent.left
        anchors.right: elapsed.left
        anchors.rightMargin: Style.space(8)
        textFormat: Text.PlainText
        text: panel.logMode ? panel.strings.t("session.logTitle")
          : panel.strings.t("session.stopTitle").arg(panel.sessionTitle)
        color: panel.foreground
        font.family: panel.fontFamily
        font.pixelSize: Style.font.body
        font.bold: true
        elide: Text.ElideRight
      }

      Text {
        id: elapsed
        anchors.right: parent.right
        anchors.verticalCenter: heading.verticalCenter
        visible: !panel.logMode
        textFormat: Text.PlainText
        text: panel.elapsedText
        color: panel.mutedForeground
        font.family: panel.fontFamily
        font.pixelSize: Style.font.bodySmall
      }
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
              iconText: modelData.color !== "" ? "\u2003" : ""
              iconSize: Style.font.caption
              leftAlign: true
              readonly property bool chosen: panel.targetKey === key
              bordered: true
              background: chosen ? Style.selectedStateColor(panel.foreground, Commons.Color.accent) : "transparent"
              foreground: chosen && !hot ? Commons.Color.background : panel.foreground
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
      visible: panel.logMode && panel.targets.length > 0
      spacing: Style.space(4)

      Text {
        anchors.verticalCenter: parent.verticalCenter
        width: Style.space(96)
        textFormat: Text.PlainText
        text: panel.strings.t("session.minutes")
        color: panel.minutes === null && panel.minutesText !== "" ? Commons.Color.urgent : panel.foreground
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
        onAccepted: panel.save()
        Keys.onEscapePressed: function(event) {
          panel.cancelRequested()
          event.accepted = true
        }
      }

      Repeater {
        model: [15, 30, 60]

        Button {
          required property int modelData
          width: Style.space(34)
          height: Style.space(24)
          text: String(modelData)
          readonly property bool chosen: panel.minutes === modelData
          bordered: true
          background: chosen ? Style.selectedStateColor(panel.foreground, Commons.Color.accent) : "transparent"
          foreground: chosen && !hot ? Commons.Color.background : panel.foreground
          fontFamily: panel.fontFamily
          fontSize: Style.font.bodySmall
          horizontalPadding: 0
          verticalPadding: 0
          onClicked: {
            minutesField.text = String(modelData)
            panel.minutesText = String(modelData)
          }
        }
      }
    }

    // ---- metrics ----------------------------------------------------------

    Text {
      visible: panel.requiredMetrics.length > 0 && panel.optionalMetrics.length > 0
      textFormat: Text.PlainText
      text: panel.strings.t("session.required")
      color: panel.mutedForeground
      font.family: panel.fontFamily
      font.pixelSize: Style.font.caption
    }

    Repeater {
      id: requiredRepeater
      model: panel.requiredMetrics

      MetricField {
        required property var modelData
        width: form.width
        strings: panel.strings
        metric: modelData
        value: panel.values[String(modelData.metric_id)]
        missing: panel.missing.indexOf(String(modelData.metric_id)) >= 0
        fontFamily: panel.fontFamily
        foreground: panel.foreground
        mutedForeground: panel.mutedForeground
        onValueEdited: function(value) { panel.setValue(modelData.metric_id, value) }
        onEscapePressed: panel.cancelRequested()
        onTextFocusChanged: panel.metricFocus = textFocus
      }
    }

    Text {
      visible: panel.optionalMetrics.length > 0
      textFormat: Text.PlainText
      text: panel.strings.t("session.optional")
      color: panel.mutedForeground
      font.family: panel.fontFamily
      font.pixelSize: Style.font.caption
    }

    Repeater {
      id: optionalRepeater
      model: panel.optionalMetrics

      MetricField {
        required property var modelData
        width: form.width
        strings: panel.strings
        metric: modelData
        value: panel.values[String(modelData.metric_id)]
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
      onAccepted: panel.save()
      Keys.onEscapePressed: function(event) {
        panel.cancelRequested()
        event.accepted = true
      }
    }

    Text {
      visible: text !== ""
      width: parent.width
      textFormat: Text.PlainText
      text: panel.errorText
      color: Commons.Color.urgent
      font.family: panel.fontFamily
      font.pixelSize: Style.font.caption
      wrapMode: Text.Wrap
      maximumLineCount: 3
      elide: Text.ElideRight
    }

    Item {
      width: parent.width
      implicitHeight: saveButton.implicitHeight

      Text {
        anchors.left: parent.left
        anchors.right: actions.left
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        visible: panel.blocker.trim() !== "" && panel.errorText === ""
        textFormat: Text.PlainText
        text: panel.blocker
        color: panel.mutedForeground
        font.family: panel.fontFamily
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
      }

      Row {
        id: actions
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
          id: saveButton
          visible: !panel.logMode || panel.targets.length > 0
          text: panel.busy ? panel.strings.t("session.saving") : panel.strings.t("session.save")
          enabled: !panel.busy && panel.blocker === ""
          opacity: enabled ? 1 : 0.45
          bordered: true
          foreground: panel.foreground
          fontFamily: panel.fontFamily
          fontSize: Style.font.bodySmall
          onClicked: panel.save()
        }
      }
    }
  }
}
