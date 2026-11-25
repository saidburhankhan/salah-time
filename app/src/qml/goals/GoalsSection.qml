import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// The Goals section of the project, epic and task editors: the scope's
// active goals with progress, and a collapsed form to add one. Goals are
// optional; with none the section only says so.
ColumnLayout {
    id: section
    objectName: "goalsSection"
    property var win
    // "project", "epic" or "item"; scopeId is that row's id.
    property string scopeKind: "project"
    property string scopeId: ""
    property var taskMetrics: null
    property bool formOpen: false
    readonly property var rows: (backend.goals || []).filter(function(goal) {
        var owner = scopeKind === "item" ? goal.item_id : scopeKind === "epic" ? goal.epic_id : goal.project_id
        return scopeId !== "" && String(owner || "") === scopeId
    })
    // Metrics a goal here may measure: the task's own asks, else its project's
    // asked metrics; work without a project offers the built-ins.
    readonly property var metricChoices: {
        var projectId = ""
        var asked = null
        if (scopeKind === "project") {
            projectId = scopeId
        } else if (scopeKind === "epic") {
            var epic = win.epicById(scopeId)
            projectId = epic ? String(epic.project_id || "") : ""
        } else {
            var task = (backend.trackingItems || []).find(function(entry) { return String(entry.id) === scopeId })
            var effective = task && task.metrics != null ? task.metrics : taskMetrics
            if (effective != null) {
                return effective.map(function(link) {
                    return win.metricById(link.metric_id) || Object.assign({}, link, {id: link.metric_id})
                }).filter(function(metric) { return !metric.archived })
            }
            projectId = task ? String(task.project_id || "") : ""
            asked = task && task.metric_ids ? task.metric_ids : null
        }
        var ids = []
        if (asked) {
            ids = asked.map(String)
        } else if (projectId !== "") {
            var project = win.projectById(projectId)
            ids = ((project && project.metrics) || []).map(function(link) { return String(link.metric_id) })
        } else {
            ids = (backend.metrics || []).filter(function(m) { return m.builtin }).map(function(m) { return String(m.id) })
        }
        return ids.map(function(id) { return win.metricById(id) })
                  .filter(function(m) { return !!m && !m.archived })
    }
    spacing: 8

    onScopeIdChanged: formOpen = false

    RowLayout {
        Layout.fillWidth: true
        Label {
            Layout.fillWidth: true
            text: qsTr("Goals")
            color: section.win.muted
            font.pixelSize: 13
        }
        AppButton {
            objectName: "goalAddToggle"
            win: section.win
            implicitHeight: 34
            text: section.formOpen ? qsTr("Cancel") : qsTr("Add goal")
            enabled: section.scopeId !== ""
            onClicked: section.formOpen = !section.formOpen
        }
    }
    Label {
        visible: section.rows.length === 0 && !section.formOpen
        Layout.fillWidth: true
        text: qsTr("No goals. A goal is an optional target for a day or a week.")
        color: section.win.muted
        font.pixelSize: 12
        wrapMode: Text.WordWrap
    }
    Repeater {
        model: section.rows
        GoalRow {
            required property var modelData
            win: section.win
            goal: modelData
        }
    }
    GoalForm {
        onCancelled: section.formOpen = false
        visible: section.formOpen
        Layout.fillWidth: true
        win: section.win
        projectId: section.scopeKind === "project" ? section.scopeId : ""
        epicId: section.scopeKind === "epic" ? section.scopeId : ""
        itemId: section.scopeKind === "item" ? section.scopeId : ""
        metricChoices: section.metricChoices
        onAdded: section.formOpen = false
    }
}
