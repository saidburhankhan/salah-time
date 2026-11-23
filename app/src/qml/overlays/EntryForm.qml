import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

Popup {
    id: entryPopup
    objectName: "entryPopup"
    property var win
    property var item: ({})
    property string block: item.block || "fajr"
    property string repeat: item.repeat || "none"
    property var days: item.weekdays || []
    property string projectId: ""
    // Optional, and always one of the chosen project's epics.
    property string epicId: ""
    readonly property var epicChoices: {
        var list = win.projectEpics(projectId, false)
        var current = win.epicById(epicId)
        if (current && String(current.project_id) === projectId && current.status === "finished")
            list.push(current)
        return list
    }
    property var metricIds: null
    property bool metricIdsEdited: false
    readonly property bool projectChanged: !!item.id && projectId !== (item.projectId || "")
    readonly property var projectMetrics: win.promptMetrics(projectId, {}, null, false)
    readonly property var askChoices: {
        var list = projectMetrics.slice()
        var ids = projectChanged ? [] : (item.metricIds || [])
        for (var i = 0; i < ids.length; i++) {
            var id = ids[i]
            var metric = win.metricById(id)
            if (metric && metric.builtin && !list.some(function(entry) { return entry.metric_id === id }))
                list.push({ metric_id: id, name: metric.name })
        }
        return list
    }
    property bool confirming: false
    readonly property bool weeklyValid: repeat !== "weekly" || days.length > 0
    readonly property var preview: backend.preview
    visible: win ? win.entryVisible : false
    modal: true
    focus: true
    anchors.centerIn: Overlay.overlay
    width: win ? Math.min(1100, win.width - 48) : 0
    height: win ? Math.min(820, win.height - 48) : 0
    padding: 0
    closePolicy: Popup.NoAutoClose
    Overlay.modal: Rectangle { color: "#d9000000" }
    background: null
    onOpened: {
        block = item.block || "fajr"
        repeat = item.repeat || "none"
        days = item.weekdays || []
        projectId = item.projectId || ""
        epicId = item.epicId || ""
        metricIds = item.metricIds || null
        metricIdsEdited = false
        confirming = false
        backend.loadSettings()
        entryFields.initialize(item.title || "", item.plannedMinutes ? String(item.plannedMinutes) : "")
        refreshPreview()
    }
    function refreshPreview() {
        if (!weeklyValid) return
        backend.previewItem(draft())
    }
    onBlockChanged: previewTimer.restart()
    onRepeatChanged: previewTimer.restart()
    onDaysChanged: previewTimer.restart()
    Timer {
        id: previewTimer
        interval: 180
        onTriggered: entryPopup.refreshPreview()
    }
    Connections {
        target: entryFields
        function onGoalTextChanged() { previewTimer.restart() }
    }
    function draft() {
        var goal = Number(entryFields.goalText)
        var result = {
            id: item.id || "",
            title: entryFields.titleText,
            block: block,
            project_id: projectId,
            planned_minutes: goal > 0 ? goal : 0,
            repeat: repeat,
            weekdays: repeat === "weekly" ? days : []
        }
        if (metricIdsEdited && !projectChanged)
            result.metric_ids = metricIds
        // Sent when an epic is chosen, or to clear the one the task had.
        if (epicId !== "" || !!item.epicId)
            result.epic_id = epicId
        return result
    }
    function selectProject(id) {
        if (projectId === id)
            return
        projectId = id
        epicId = id === (item.projectId || "") ? (item.epicId || "") : ""
        metricIds = projectChanged ? null : (item.metricIds || null)
        metricIdsEdited = false
    }
    function asksMetric(id) {
        return metricIds === null
            ? projectMetrics.some(function(metric) { return metric.metric_id === id })
            : metricIds.indexOf(id) >= 0
    }
    function toggleMetric(id) {
        if (projectChanged)
            return
        var ids = askChoices.filter(function(metric) { return asksMetric(metric.metric_id) })
                            .map(function(metric) { return metric.metric_id })
        var index = ids.indexOf(id)
        if (index >= 0)
            ids.splice(index, 1)
        else
            ids.push(id)
        metricIds = ids
        metricIdsEdited = true
    }
    function resetMetrics() {
        metricIds = null
        metricIdsEdited = true
    }
    function submitTitle() {
        if (!entryFields.titleText.trim() || !weeklyValid) return
        var d = draft()
        win.maybeToast(d)
        backend.saveItem(d)
        win.entryVisible = false
    }
    function toggleDay(d) {
        var a = days.slice()
        var i = a.indexOf(d)
        if (i >= 0) a.splice(i, 1)
        else a.push(d)
        days = a
    }
    function selectRepeat(id) {
        if (id === "weekdays") {
            repeat = "weekly"
            days = ["mon", "tue", "wed", "thu", "fri"]
        } else {
            repeat = id
            if (id === "weekly" && days.length === 0) {
                var selected = Date.fromLocaleString(Qt.locale(), backend.selectedDay, "yyyy-MM-dd")
                days = [entryPopup.win.weekdayId(selected)]
            }
        }
    }
    Rectangle {
        anchors.fill: parent
        color: "#151411"
        radius: Math.max(8, theme.radius)
        border.width: 1
        border.color: entryPopup.win.border
    }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 1
        spacing: 0
        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 72
            Layout.leftMargin: 22
            Layout.rightMargin: 22
            Label {
                text: entryPopup.item.id ? qsTr("Edit entry") : qsTr("New entry")
                color: entryPopup.win.text
                font.pixelSize: 20
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }
            AppButton {
                win: entryPopup.win
                visible: !!entryPopup.item.id
                text: entryPopup.confirming ? qsTr("Confirm delete") : qsTr("Delete")
                onClicked: {
                    if (entryPopup.confirming) {
                        backend.removeItem(entryPopup.item.id)
                        entryPopup.win.entryVisible = false
                    } else {
                        entryPopup.confirming = true
                    }
                }
            }
            AppButton {
                win: entryPopup.win
                text: qsTr("Cancel")
                onClicked: entryPopup.win.entryVisible = false
            }
            AppButton {
                win: entryPopup.win
                text: qsTr("Save & add another")
                enabled: entryFields.titleText.trim() !== "" && entryPopup.weeklyValid
                onClicked: {
                    var d = entryPopup.draft()
                    entryPopup.win.maybeToast(d)
                    backend.saveItem(d)
                    entryPopup.win.editItem = ({})
                    entryPopup.metricIds = null
                    entryPopup.metricIdsEdited = false
                    entryFields.clearTitle()
                }
            }
            AppButton {
                win: entryPopup.win
                text: qsTr("Save  ↵")
                primary: true
                enabled: entryFields.titleText.trim() !== "" && entryPopup.weeklyValid
                onClicked: {
                    var d = entryPopup.draft()
                    entryPopup.win.maybeToast(d)
                    backend.saveItem(d)
                    entryPopup.win.entryVisible = false
                }
            }
        }
        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: entryPopup.win.border
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: 22
            spacing: 26
            EntryFields {
                id: entryFields
                win: entryPopup.win
                form: entryPopup
            }
            ColumnLayout {
                Layout.preferredWidth: 340
                Layout.minimumWidth: 280
                Layout.maximumWidth: 360
                Layout.fillHeight: true
                Rectangle {
                    Layout.fillWidth: true
                    height: 285
                    radius: theme.radius
                    color: entryPopup.win.panel
                    border.color: entryPopup.win.border
                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 22
                        spacing: 14
                        Label {
                            text: qsTr("Preview")
                            color: entryPopup.win.text
                            font.pixelSize: 18
                            font.weight: Font.DemiBold
                        }
                        Rectangle {
                            Layout.fillWidth: true
                            height: 1
                            color: entryPopup.win.border
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                text: qsTr("Next occurrence")
                                color: entryPopup.win.muted
                                Layout.fillWidth: true
                            }
                            Label {
                                text: entryPopup.preview.nextOccurrence
                                    ? backend.gregorianHeader(entryPopup.preview.nextOccurrence)
                                    : "—"
                                color: entryPopup.win.text
                                font.family: entryPopup.win.mono
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                text: qsTr("Occurrences this month")
                                color: entryPopup.win.muted
                                Layout.fillWidth: true
                            }
                            Label {
                                text: entryPopup.preview.valid
                                    ? entryPopup.preview.occurrencesThisMonth
                                    : "—"
                                color: entryPopup.win.text
                                font.family: entryPopup.win.mono
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                text: qsTr("Schedule")
                                color: entryPopup.win.muted
                                Layout.fillWidth: true
                            }
                            Label {
                                text: entryPopup.preview.scheduleText || "—"
                                color: entryPopup.win.text
                                font.family: entryPopup.win.mono
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                text: qsTr("Planned")
                                color: entryPopup.win.muted
                                Layout.fillWidth: true
                            }
                            Label {
                                text: entryFields.goalText ? qsTr("%1 min").arg(entryFields.goalText) : qsTr("None")
                                color: entryPopup.win.text
                                font.family: entryPopup.win.mono
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                text: qsTr("Project")
                                color: entryPopup.win.muted
                                Layout.fillWidth: true
                            }
                            Rectangle {
                                visible: !!entryPopup.win.projectById(entryPopup.projectId)
                                width: 10
                                height: 10
                                radius: 5
                                color: entryPopup.win.projectColor(entryPopup.projectId)
                            }
                            Label {
                                text: entryPopup.win.projectName(entryPopup.projectId) || qsTr("None")
                                textFormat: Text.PlainText
                                color: entryPopup.win.text
                                font.family: entryPopup.win.mono
                                elide: Text.ElideRight
                                Layout.maximumWidth: 180
                            }
                        }
                    }
                }
                Item { Layout.fillHeight: true }
            }
        }
    }
}
