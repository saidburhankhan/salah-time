import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../goals"
import "../resources"
import "../statistics"

// One epic: its project, status and totals, minutes per day, the tasks and
// routines that use it, and its notes. Opened from a project's epic list or a
// Statistics row; it is not a sidebar entry. Everything on it is optional.
ScrollView {
    id: epicPage
    objectName: "epicPage"
    property var win
    readonly property var epic: win.epicById(win.epicId)
    readonly property var project: epic ? win.projectById(epic.project_id) : null
    readonly property color dot: project ? Qt.color(String(project.color || "#78909c")) : Qt.color("#5f5b53")
    readonly property bool paused: !!epic?.paused || !!project?.archived
    readonly property bool finished: !!epic && epic.status === "finished"
    readonly property var tasks: ((paused ? backend.reportItems : backend.trackingItems) || []).filter(function(item) {
        return String(item.epic_id || "") === String(win.epicId)
    })
    // stats.series (minutes, split none) as the points the chart draws.
    readonly property var minutesSeries: {
        var data = backend.epicSeries || {}
        var periods = data.periods || []
        var rows = data.series || []
        var points = []
        for (var i = 0; i < periods.length; i++) {
            var point = rows.length > 0 ? (rows[0].points || [])[i] : null
            points.push({ day: String(periods[i]), value: point ? point.value : null })
        }
        return { name: qsTr("Minutes per day"), unit: "min", rollup: "sum", points: points }
    }
    clip: true
    contentWidth: availableWidth

    function open() {
        backend.loadSettings()
        backend.loadTrackingItems()
        backend.loadReportItems()
        backend.loadEpicPage(win.epicId)
    }
    function setStatus(status) {
        if (epic)
            backend.saveEntity("epic", { id: String(epic.id), status: status })
    }
    function logTime() {
        if (!epic || paused)
            return
        win.openSessionPrompt({
            mode: "log",
            projectId: String(epic.project_id),
            epicId: String(epic.id),
            title: String(epic.name || ""),
            color: project ? String(project.color || "") : ""
        })
    }
    function openSession(session) { win.editReportSession(session) }
    function taskText(item) {
        return item.repeat === "none" ? qsTr("one-time") : String(item.schedule_text || item.repeat || "")
    }

    Connections {
        target: backend
        function onReportsInvalidated() {
            if (epicPage.visible) {
                backend.loadTrackingItems()
                backend.loadReportItems()
                backend.loadEpicPage(epicPage.win.epicId)
            }
        }
        function onSessionWriteFinished(ok, message) {
            if (ok && epicPage.visible)
                epicPage.open()
        }
        function onEntitySaved(kind, id) {
            if (kind === "epic" && epicPage.visible && !epicPage.epic)
                epicPage.win.page = epicPage.win.epicBackPage
        }
    }

    ColumnLayout {
        width: epicPage.availableWidth
        spacing: 16

        RowLayout {
            Layout.fillWidth: true
            AppButton {
                objectName: "epicBackButton"
                win: epicPage.win
                text: qsTr("←  Back")
                onClicked: epicPage.win.page = epicPage.win.epicBackPage
            }
            Item {
                Layout.fillWidth: true
            }
        }

        Label {
            visible: !epicPage.epic
            Layout.fillWidth: true
            Layout.topMargin: 24
            text: qsTr("This epic is not available. It may have been removed.")
            color: epicPage.win.muted
            horizontalAlignment: Text.AlignHCenter
        }

        Card {
            visible: !!epicPage.epic
            win: epicPage.win
            Layout.fillWidth: true
            Layout.preferredHeight: head.implicitHeight + 40
            RowLayout {
                id: head
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 20
                spacing: 14
                Rectangle {
                    width: 14
                    height: 14
                    radius: 7
                    color: epicPage.dot
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4
                    Label {
                        objectName: "epicTitle"
                        Layout.fillWidth: true
                        text: epicPage.epic ? String(epicPage.epic.name || "") : ""
                        textFormat: Text.PlainText
                        color: epicPage.win.text
                        font.pixelSize: 24
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }
                    Label {
                        objectName: "epicSubtitle"
                        Layout.fillWidth: true
                        text: (epicPage.project ? String(epicPage.project.name || "") : qsTr("No project"))
                            + "  ·  " + (epicPage.paused ? qsTr("Project archived")
                                             : epicPage.finished ? qsTr("Finished") : qsTr("Active"))
                        textFormat: Text.PlainText
                        color: epicPage.win.muted
                        font.pixelSize: 13
                    }
                    Label {
                        visible: !!epicPage.epic && !!epicPage.epic.note
                        Layout.fillWidth: true
                        text: epicPage.epic ? String(epicPage.epic.note || "") : ""
                        textFormat: Text.PlainText
                        color: epicPage.win.muted
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                    }
                }
                ResourceChip {
                    objectName: "epicResources"
                    win: epicPage.win
                    selected: epicPage.win.resourceTargetId === epicPage.win.epicId
                    onClicked: epicPage.win.openResources("epic", epicPage.win.epicId,
                                                          epicPage.epic ? String(epicPage.epic.name || "") : "")
                }
                AppButton {
                    objectName: "epicLogButton"
                    win: epicPage.win
                    text: qsTr("Log time")
                    enabled: !epicPage.finished && !epicPage.paused
                    onClicked: epicPage.logTime()
                }
                AppButton {
                    objectName: "epicFinishButton"
                    win: epicPage.win
                    primary: !epicPage.finished
                    text: epicPage.finished ? qsTr("Reopen") : qsTr("Finish")
                    onClicked: epicPage.setStatus(epicPage.finished ? "active" : "finished")
                }
            }
        }

        RowLayout {
            visible: !!epicPage.epic
            Layout.fillWidth: true
            spacing: 12
            SummaryTile {
                win: epicPage.win
                label: qsTr("Total time")
                value: epicPage.epic ? epicPage.win.durationText(epicPage.epic.total_minutes) : ""
                detail: epicPage.epic ? qsTr("%1 min in all").arg(Number(epicPage.epic.total_minutes || 0)) : ""
            }
            SummaryTile {
                win: epicPage.win
                label: qsTr("Sessions")
                value: epicPage.epic ? String(epicPage.epic.sessions || 0) : ""
                detail: qsTr("logged against this epic")
            }
            SummaryTile {
                win: epicPage.win
                label: qsTr("Days worked")
                value: epicPage.epic ? String(epicPage.epic.days_worked || 0) : ""
                detail: epicPage.epic && epicPage.epic.last_worked
                    ? qsTr("last %1").arg(epicPage.win.shortDate(epicPage.epic.last_worked)) : qsTr("nothing yet")
            }
            SummaryTile {
                win: epicPage.win
                label: qsTr("Tasks")
                value: epicPage.epic ? String(Number(epicPage.epic.open_tasks || 0) + Number(epicPage.epic.done_tasks || 0)) : ""
                detail: epicPage.epic ? qsTr("%1 open, %2 done").arg(epicPage.epic.open_tasks || 0).arg(epicPage.epic.done_tasks || 0) : ""
            }
        }

        LineChart {
            objectName: "epicMinutesChart"
            visible: !!epicPage.epic
            Layout.fillWidth: true
            win: epicPage.win
            asLine: true
            series: epicPage.minutesSeries
        }

        Card {
            visible: !!epicPage.epic
            win: epicPage.win
            Layout.fillWidth: true
            Layout.preferredHeight: taskColumn.implicitHeight + 36
            ColumnLayout {
                id: taskColumn
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 18
                spacing: 8
                Label {
                    text: qsTr("Tasks and routines")
                    color: epicPage.win.text
                    font.pixelSize: 15
                    font.weight: Font.Medium
                }
                Label {
                    visible: epicPage.tasks.length === 0
                    Layout.fillWidth: true
                    text: qsTr("No task or routine uses this epic. Pick it in a task's editor.")
                    color: epicPage.win.muted
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                Repeater {
                    model: epicPage.tasks
                    RowLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        spacing: 12
                        Label {
                            Layout.fillWidth: true
                            text: String(modelData.title || "")
                            textFormat: Text.PlainText
                            color: epicPage.win.text
                            font.pixelSize: 14
                            elide: Text.ElideRight
                        }
                        Label {
                            text: epicPage.taskText(modelData)
                            textFormat: Text.PlainText
                            color: epicPage.win.muted
                            font.family: epicPage.win.mono
                            font.pixelSize: 12
                        }
                    }
                }
            }
        }

        Card {
            visible: !!epicPage.epic
            win: epicPage.win
            Layout.fillWidth: true
            Layout.preferredHeight: goalsColumn.implicitHeight + 36
            GoalsSection {
                id: goalsColumn
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 18
                win: epicPage.win
                scopeKind: "epic"
                scopeId: epicPage.epic ? String(epicPage.epic.id) : ""
            }
        }

        Label {
            visible: !!epicPage.epic
            text: qsTr("Notes")
            color: epicPage.win.text
            font.pixelSize: 15
            font.weight: Font.Medium
        }
        Label {
            visible: !!epicPage.epic && (backend.epicSessions || []).length === 0
            Layout.fillWidth: true
            text: qsTr("No notes on this epic yet.")
            color: epicPage.win.muted
            font.pixelSize: 13
        }
        Repeater {
            model: epicPage.epic ? (backend.epicSessions || []) : []
            NoteRow {
                required property var modelData
                win: epicPage.win
                session: modelData
                onOpen: function(session) { epicPage.openSession(session) }
            }
        }
        Item {
            Layout.preferredHeight: 12
        }
    }
}
