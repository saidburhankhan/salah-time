import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../goals"

// Every active goal with its progress for today and this week, and a
// history table per goal. Goals for a project, epic or task are added in
// their editors; this page adds goals for all work. All of it is optional.
ScrollView {
    id: goalsPage
    objectName: "goalsPage"
    property var win
    property string historyId: ""
    readonly property var dayGoals: (backend.goals || []).filter(function(goal) { return goal.period === "day" })
    readonly property var weekGoals: (backend.goals || []).filter(function(goal) { return goal.period === "week" })
    readonly property var allMetrics: (backend.metrics || []).filter(function(m) { return m.builtin && !m.archived })
    clip: true
    contentWidth: availableWidth

    function open() {
        backend.loadSettings()
        backend.loadTrackingItems()
    }
    function scopeText(goal) {
        if (goal.item_id) {
            var task = (backend.trackingItems || []).find(function(entry) { return String(entry.id) === String(goal.item_id) })
            return task ? String(task.title || "") : qsTr("Task")
        }
        if (goal.epic_id) {
            var epic = win.epicById(goal.epic_id)
            var project = epic ? win.projectName(epic.project_id) : ""
            return epic ? String(epic.name || "") + (project !== "" ? "  ·  " + project : "") : qsTr("Epic")
        }
        if (goal.project_id)
            return win.projectName(goal.project_id) || qsTr("Project")
        return qsTr("All work")
    }
    function toggleHistory(goal) {
        historyId = historyId === String(goal.id) ? "" : String(goal.id)
        backend.loadGoalHistory(historyId)
    }
    onVisibleChanged: {
        if (!visible) {
            historyId = ""
            backend.loadGoalHistory("")
        }
    }

    Connections {
        target: backend
        function onGoalWriteFinished(ok, message) {
            // An edit replaces the goal with a successor id; close the table.
            if (ok && goalsPage.historyId !== "") {
                goalsPage.historyId = ""
                backend.loadGoalHistory("")
            }
        }
    }

    ColumnLayout {
        width: goalsPage.availableWidth
        spacing: 14

        Label {
            Layout.fillWidth: true
            text: qsTr("Optional targets for a day or a week. A goal's target never changes for past periods: editing it starts a new one.")
            color: goalsPage.win.muted
            font.pixelSize: 14
            wrapMode: Text.WordWrap
        }
        Label {
            objectName: "goalsEmpty"
            visible: (backend.goals || []).length === 0
            Layout.fillWidth: true
            Layout.topMargin: 8
            text: qsTr("No goals yet. Add one below, or in a project, epic or task editor.")
            color: goalsPage.win.muted
            font.pixelSize: 14
        }

        Repeater {
            model: [
                { id: "day", title: qsTr("Today"), rows: goalsPage.dayGoals },
                { id: "week", title: qsTr("This week"), rows: goalsPage.weekGoals }
            ]
            ColumnLayout {
                required property var modelData
                visible: modelData.rows.length > 0
                Layout.fillWidth: true
                spacing: 8
                Label {
                    text: modelData.title + "  ·  " + qsTr("%1 of %2 met")
                        .arg(modelData.rows.filter(function(g) { return g.progress && g.progress.met }).length)
                        .arg(modelData.rows.length)
                    textFormat: Text.PlainText
                    color: goalsPage.win.text
                    font.pixelSize: 15
                    font.weight: Font.Medium
                }
                Repeater {
                    model: modelData.rows
                    ColumnLayout {
                        id: entry
                        required property var modelData
                        Layout.fillWidth: true
                        spacing: 6
                        GoalRow {
                            win: goalsPage.win
                            goal: entry.modelData
                            scopeText: goalsPage.scopeText(entry.modelData)
                            showHistoryButton: true
                            historyOpen: goalsPage.historyId === String(entry.modelData.id)
                            onHistoryToggled: goalsPage.toggleHistory(entry.modelData)
                        }
                        GoalHistory {
                            visible: goalsPage.historyId === String(entry.modelData.id)
                            Layout.fillWidth: true
                            Layout.leftMargin: 12
                            Layout.rightMargin: 12
                            Layout.bottomMargin: 8
                            win: goalsPage.win
                            goal: entry.modelData
                            rows: backend.goalHistoryId === String(entry.modelData.id) ? (backend.goalHistory || []) : []
                        }
                    }
                }
            }
        }

        Card {
            win: goalsPage.win
            Layout.fillWidth: true
            Layout.preferredHeight: addColumn.implicitHeight + 36
            ColumnLayout {
                id: addColumn
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 18
                spacing: 10
                Label {
                    text: qsTr("Add a goal for all work")
                    color: goalsPage.win.text
                    font.pixelSize: 15
                    font.weight: Font.Medium
                }
                GoalForm {
                    objectName: "goalsPageForm"
                    Layout.fillWidth: true
                    win: goalsPage.win
                    metricChoices: goalsPage.allMetrics
                }
            }
        }
        Item {
            Layout.preferredHeight: 12
        }
    }
}
