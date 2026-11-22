import QtQuick
import qs.Commons
import qs.Ui

Rectangle {
  id: panel
  property var strings
  property var metrics: []
  property var projects: []
  property bool logMode: false
  property string selectedProjectId: ""
  property var values: ({})
  property var missing: []
  readonly property var effectiveMetrics: {
    if (!logMode) return metrics || []
    for (var i = 0; i < projects.length; ++i) {
      if (String(projects[i].id || "") !== selectedProjectId) continue
      return (projects[i].metrics || []).map(function(binding) {
        var metric = Object.assign({}, binding.metric || {})
        metric.metric_id = String(binding.metric_id || metric.id || "")
        metric.required = binding.required === true
        return metric
      })
    }
    return []
  }
  property string fontFamily
  property color foreground
  property color mutedForeground
  signal saveRequested(string projectId, var values, var minutes, string note)
  signal cancelRequested()

  function safeText(value) {
    return String(value === undefined || value === null ? "" : value)
      .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
  }

  function reset() {
    values = ({})
    missing = []
    selectedProjectId = projects.length ? String(projects[0].id || "") : ""
    minutesField.text = ""
    noteField.text = ""
  }
  function markMissing(ids) { missing = (ids || []).map(function(id) { return String(id) }) }
  function setValue(id, value) {
    var copy = {}
    for (var key in values) copy[key] = values[key]
    copy[String(id)] = value
    values = copy
  }
  function complete() {
    for (var i = 0; i < effectiveMetrics.length; i++)
      if (effectiveMetrics[i].required === true && values[String(effectiveMetrics[i].metric_id)] === undefined) return false
    return !logMode || selectedProjectId !== ""
  }

  width: parent ? parent.width : 0
  implicitHeight: form.implicitHeight + Style.space(20)
  radius: Style.cornerRadius
  color: Style.hoverFillFor(foreground, Commons.Color.accent)

  Column {
    id: form
    anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
    anchors.margins: Style.space(10)
    spacing: Style.space(7)

    Text {
      textFormat: Text.PlainText
      text: panel.logMode ? "+ Log session" : "Finish session"
      color: panel.foreground; font.family: panel.fontFamily; font.bold: true
    }

    Flow {
      visible: panel.logMode
      width: parent.width; spacing: Style.space(5)
      Repeater {
        model: panel.projects
        Button {
          required property var modelData
          text: panel.safeText(modelData.name || "")
          foreground: panel.foreground; fontFamily: panel.fontFamily
          bordered: panel.selectedProjectId === String(modelData.id || "")
          onClicked: panel.selectedProjectId = String(modelData.id || "")
        }
      }
    }

    TextField {
      id: minutesField
      visible: panel.logMode
      width: parent.width
      placeholderText: "Minutes"
      validator: IntValidator { bottom: 1; top: 1440 }
      foreground: panel.foreground; font.family: panel.fontFamily
    }

    Repeater {
      model: panel.effectiveMetrics
      Column {
        required property var modelData
        width: form.width; spacing: Style.space(3)
        Text {
          textFormat: Text.PlainText
          text: String(modelData.name || "") + (modelData.required ? " *" : "")
          color: panel.missing.indexOf(String(modelData.metric_id)) >= 0
            ? Commons.Color.error : panel.mutedForeground
          font.family: panel.fontFamily
        }
        Row {
          visible: String(modelData.kind || "") === "scale"
          spacing: Style.space(4)
          Repeater {
            model: Math.max(0, Number(modelData.scale_max) || 5)
            Button {
              required property int index
              text: String(index + 1); width: Style.space(30); height: Style.space(26)
              bordered: panel.values[String(modelData.metric_id)] === index + 1
              foreground: panel.foreground; fontFamily: panel.fontFamily
              onClicked: panel.setValue(modelData.metric_id, index + 1)
            }
          }
        }
        TextField {
          visible: String(modelData.kind || "") === "count"
          width: parent.width; placeholderText: String(modelData.unit || "Count")
          validator: DoubleValidator { bottom: 0 }
          foreground: panel.foreground; font.family: panel.fontFamily
          onTextChanged: if (text !== "") panel.setValue(modelData.metric_id, Number(text))
        }
        Button {
          visible: String(modelData.kind || "") === "check"
          text: panel.values[String(modelData.metric_id)] === 1 ? "✓ Yes" : "○ No"
          foreground: panel.foreground; fontFamily: panel.fontFamily
          onClicked: panel.setValue(modelData.metric_id,
            panel.values[String(modelData.metric_id)] === 1 ? 0 : 1)
        }
      }
    }

    TextField {
      id: noteField
      width: parent.width; placeholderText: "Optional note"; maximumLength: 240
      foreground: panel.foreground; font.family: panel.fontFamily
    }

    Row {
      spacing: Style.space(6)
      Button { text: "Cancel"; foreground: panel.mutedForeground; fontFamily: panel.fontFamily; onClicked: panel.cancelRequested() }
      Button {
        text: "Save"; bordered: true; enabled: panel.complete()
        foreground: panel.foreground; fontFamily: panel.fontFamily
        onClicked: panel.saveRequested(panel.selectedProjectId, panel.values,
          panel.logMode && minutesField.text !== "" ? Number(minutesField.text) : null, noteField.text)
      }
    }
  }
}
