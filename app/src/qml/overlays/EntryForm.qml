import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../goals"
import "../resources"

Popup {
    id: entryPopup
    objectName: "entryPopup"
    property var win
    property var item: ({})
    readonly property bool backlog: !!item.backlog
    property string block: item.block || "fajr"
    property string repeat: item.repeat || "none"
    readonly property bool existingRoutine: !!item.id && !backlog
        && (item.repeat === "daily" || item.repeat === "weekly")
    readonly property bool convertingRoutine: existingRoutine && repeat === "none"
    property string endDay: ""
    property bool endDayEdited: false
    property string routineSaveId: ""
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
    property var tagOverrides: null
    // Routine report rows may need the item list to supply their current tags.
    readonly property var savedTagIds: item.tagIds ?? (item.id
        ? (backend.trackingItems || []).find(function(task) { return task.id === item.id })?.tag_ids
        : undefined)
    readonly property var tagIds: tagOverrides ?? (item.id
        ? (savedTagIds ?? [])
        : (win.projectById(projectId)?.tag_ids || []))
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
    property var pendingTask: null
    property string createdId: ""
    property var remaining: []
    property bool saving: false
    property bool addAnother: false
    property string saveError: ""
    property bool resourceRejected: false
    readonly property bool canSubmit: !saving && !resourceRejected && !attachments.composing
        && weeklyValid && (!convertingRoutine || entryFields.conversionDateValid) && !!entryFields.titleText.trim()
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
    property double openedVersion: -1
    onOpened: {
        openedVersion = item.version ?? -1
        endDay = item.endDay || ""
        endDayEdited = false
        routineSaveId = ""
        entryFields.conversionDay = backend.todayDay
        block = item.block || "fajr"
        repeat = item.repeat || "none"
        days = item.weekdays || []
        projectId = item.projectId || ""
        epicId = item.epicId || ""
        metricIds = item.metricIds || null
        metricIdsEdited = false
        tagOverrides = null
        confirming = false
        resetAttachments()
        backend.loadSettings()
        if (item.id && item.tagIds == null)
            backend.loadTrackingItems()
        entryFields.initialize(item.title || "", item.plannedMinutes ? String(item.plannedMinutes) : "")
        refreshPreview()
    }
    function refreshPreview() {
        if (!visible || backlog || !weeklyValid || (convertingRoutine && !entryFields.conversionDateValid)) return
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
        function onConversionDayChanged() { previewTimer.restart() }
    }
    function draft() {
        var goal = Number(entryFields.goalText)
        var result = {
            id: item.id || "",
            title: entryFields.titleText,
            project_id: projectId,
            planned_minutes: goal > 0 ? goal : 0,
            repeat: repeat,
            weekdays: repeat === "weekly" ? days : []
        }
        if (item.id && openedVersion >= 0)
            result.expected_version = openedVersion
        if (convertingRoutine)
            result.date = entryFields.conversionDay
        else if (existingRoutine && endDayEdited)
            result.end_day = endDay || null
        if (backlog)
            result.backlog = true
        else
            result.block = block
        if (metricIdsEdited && !projectChanged)
            result.metric_ids = metricIds
        if (tagOverrides !== null)
            result.tag_ids = tagOverrides
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
        if (!item.id)
            tagOverrides = null
        metricIds = projectChanged ? null : (item.metricIds || null)
        metricIdsEdited = false
    }
    function asksMetric(id) {
        return metricIds === null
            ? projectMetrics.some(function(metric) { return metric.metric_id === id })
            : metricIds.indexOf(id) >= 0
    }
    function toggleTag(id) {
        var ids = Array.from(tagIds)
        var index = ids.indexOf(id)
        if (index >= 0)
            ids.splice(index, 1)
        else
            ids.push(id)
        tagOverrides = ids
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
    function resetAttachments() {
        pendingTask = null
        createdId = ""
        remaining = []
        resourceRejected = false
        saveError = ""
        saving = false
        attachments.reset()
    }
    function saveNext() {
        saving = true
        saveError = ""
        if (!createdId)
            backend.saveItem(pendingTask)
        else
            backend.attachTaskResource(createdId, remaining[0].fields, remaining[0].operation)
    }
    function advanceAttachments() {
        resourceRejected = false
        remaining = remaining.slice(1)
        if (remaining.length) saveNext()
        else finishSave(addAnother)
    }
    function correctAttachment(fields) {
        fields.kind = remaining[0].fields.kind
        remaining = [{ fields: fields, operation: backend.newOperationId() }].concat(remaining.slice(1))
        resourceRejected = false
        saveNext()
    }
    Connections {
        target: backend
        function onRoutineEditFinished(id, ok, error) {
            if (id !== entryPopup.routineSaveId) return
            entryPopup.saving = false
            entryPopup.routineSaveId = ""
            if (ok) entryPopup.finishSave(entryPopup.addAnother)
            else entryPopup.saveError = error
        }
        function onTagsLoaded() {
            if (entryPopup.pendingTask || !entryPopup.saveError || entryPopup.tagOverrides === null) return
            entryPopup.tagOverrides = entryPopup.tagOverrides.filter(id => backend.tags.some(tag => tag.id === id))
        }
        function onTaskCreateFinished(operation, id, error, rejected) {
            if (!entryPopup.pendingTask || entryPopup.pendingTask.creation_op_id !== operation) return
            if (error) {
                entryPopup.saving = false
                entryPopup.saveError = error
                if (rejected) {
                    entryPopup.pendingTask = null
                    entryPopup.remaining = []
                    backend.loadSettings()
                }
                return
            }
            entryPopup.createdId = id
            if (!entryPopup.backlog) entryPopup.win.maybeToast(entryPopup.pendingTask)
            if (entryPopup.remaining.length) entryPopup.saveNext()
            else entryPopup.finishSave(entryPopup.addAnother)
        }
        function onAttachmentFinished(operation, error, rejected) {
            if (!entryPopup.remaining.length || entryPopup.remaining[0].operation !== operation) return
            if (error) {
                entryPopup.saving = false
                entryPopup.saveError = qsTr("Task saved. Could not attach resource: %1").arg(error)
                entryPopup.resourceRejected = rejected
                return
            }
            entryPopup.advanceAttachments()
        }
    }
    function submitTitle(keepOpen) {
        if (!canSubmit) return
        if (pendingTask) { saveNext(); return }
        var d = draft()
        if (!item.id) {
            d.creation_op_id = backend.newOperationId()
            d.creation_date = backend.selectedDay
            pendingTask = d
            remaining = attachments.entries.map(fields => ({fields: fields, operation: backend.newOperationId()}))
            addAnother = keepOpen
            saveNext()
            return
        }
        if (convertingRoutine || (existingRoutine && endDayEdited)) {
            saveError = ""
            routineSaveId = item.id
            addAnother = keepOpen
            saving = true
            backend.saveItem(d)
            return
        }
        if (!backlog) win.maybeToast(d)
        backend.saveItem(d)
        finishSave(keepOpen)
    }
    function finishSave(keepOpen) {
        resetAttachments()
        if (keepOpen) {
            win.editItem = backlog ? ({ backlog: true }) : ({})
            endDay = ""
            endDayEdited = false
            metricIds = null
            metricIdsEdited = false
            tagOverrides = null
            entryFields.clearTitle()
        } else {
            win.entryVisible = false
        }
    }
    function setEndDay(day) {
        endDay = day
        endDayEdited = endDay !== (item.endDay || "")
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
                text: entryPopup.win.page === 1 ? (entryPopup.item.id ? qsTr("Edit routine") : qsTr("New routine"))
                    : entryPopup.backlog
                    ? (entryPopup.item.id ? qsTr("Edit backlog task") : qsTr("New backlog task"))
                    : (entryPopup.item.id ? qsTr("Edit entry") : qsTr("New entry"))
                color: entryPopup.win.text
                font.pixelSize: 20
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }
            AppButton {
                win: entryPopup.win
                visible: !!entryPopup.item.id
                enabled: !entryPopup.saving
                text: entryPopup.confirming ? qsTr("Confirm delete") : qsTr("Delete")
                onClicked: {
                    if (entryPopup.confirming) {
                        backend.removeItem(entryPopup.item.id, entryPopup.openedVersion)
                        entryPopup.win.entryVisible = false
                    } else {
                        entryPopup.confirming = true
                    }
                }
            }
            AppButton {
                win: entryPopup.win
                enabled: !entryPopup.saving
                text: entryPopup.createdId ? qsTr("Close") : qsTr("Cancel")
                onClicked: entryPopup.win.entryVisible = false
            }
            AppButton {
                win: entryPopup.win
                visible: !entryPopup.pendingTask
                text: qsTr("Save & add another")
                enabled: entryPopup.canSubmit
                onClicked: entryPopup.submitTitle(true)
            }
            AppButton {
                win: entryPopup.win
                visible: !entryPopup.resourceRejected
                text: entryPopup.saving ? qsTr("Saving…") : entryPopup.pendingTask && entryPopup.saveError ? qsTr("Retry") : qsTr("Save  ↵")
                primary: true
                enabled: entryPopup.canSubmit
                onClicked: entryPopup.submitTitle(false)
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
                enabled: !entryPopup.pendingTask && !entryPopup.saving
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
                    visible: !entryPopup.backlog
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
                ColumnLayout {
                    visible: entryPopup.existingRoutine && !entryPopup.convertingRoutine
                    Layout.fillWidth: true
                    enabled: !entryPopup.saving
                    Label {
                        visible: !!entryPopup.endDay || entryPopup.endDayEdited
                        Layout.fillWidth: true
                        text: entryPopup.endDay
                            ? (entryPopup.endDayEdited ? qsTr("Last day after saving · %1") : qsTr("Last day · %1"))
                                .arg(entryPopup.win.shortDate(entryPopup.endDay))
                            : qsTr("The routine will resume when saved.")
                        textFormat: Text.PlainText
                        color: entryPopup.win.muted
                        wrapMode: Text.WordWrap
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6
                        AppButton {
                            objectName: "editorEndRoutine"
                            win: entryPopup.win
                            text: entryPopup.endDay ? qsTr("Change end date") : qsTr("End routine")
                            onClicked: entryPopup.win.openRoutineEnd({
                                id: entryPopup.item.id, title: entryFields.titleText,
                                day: entryPopup.item.startDay, end_day: entryPopup.endDay
                            }, day => entryPopup.setEndDay(day))
                        }
                        AppButton {
                            visible: !!entryPopup.endDay
                            win: entryPopup.win
                            text: qsTr("Resume")
                            onClicked: entryPopup.setEndDay("")
                        }
                    }
                }
                Label {
                    visible: entryPopup.confirming && entryPopup.existingRoutine
                    Layout.fillWidth: true
                    text: qsTr("Choose End routine to stop future occurrences and keep the routine in Ended.")
                    color: entryPopup.win.muted
                    wrapMode: Text.WordWrap
                }
                AppButton {
                    objectName: "routineResources"
                    visible: entryPopup.win.page === 1 && !!entryPopup.item.id
                    win: entryPopup.win
                    text: qsTr("Notes, links & reminders")
                    onClicked: entryPopup.win.openResources("item", entryPopup.item.id, entryPopup.item.title)
                }
                // Goals for an existing task; a new task has no id to scope one to.
                ScrollView {
                    visible: !!entryPopup.item.id
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.topMargin: 6
                    contentWidth: availableWidth
                    clip: true
                    GoalsSection {
                        width: parent.width
                        win: entryPopup.win
                        scopeKind: "item"
                        taskMetrics: entryPopup.item.metrics ?? null
                        scopeId: entryPopup.item.id ? String(entryPopup.item.id) : ""
                    }
                }
                ScrollView {
                    id: newResourcesScroll
                    visible: !entryPopup.item.id
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    contentWidth: availableWidth
                    clip: true
                    ColumnLayout {
                        width: newResourcesScroll.availableWidth
                        NewTaskResources {
                            id: attachments
                            Layout.fillWidth: true
                            win: entryPopup.win
                            visible: !entryPopup.resourceRejected
                            enabled: !entryPopup.pendingTask
                        }
                        Loader {
                            Layout.fillWidth: true
                            active: entryPopup.resourceRejected
                            visible: active
                            sourceComponent: ResourceForm {
                                objectName: "rejectedAttachment"
                                win: entryPopup.win
                                submitLabel: qsTr("Save attachment")
                                cancelLabel: qsTr("Skip attachment")
                                Component.onCompleted: {
                                    load(entryPopup.remaining[0].fields)
                                    serverError = entryPopup.saveError
                                }
                                onSubmitted: fields => entryPopup.correctAttachment(fields)
                                onCancelled: entryPopup.advanceAttachments()
                            }
                        }
                    }
                }
                Label {
                    visible: !!entryPopup.saveError && !entryPopup.resourceRejected
                    Layout.fillWidth: true
                    text: entryPopup.saveError
                    textFormat: Text.PlainText
                    color: entryPopup.win.text
                    wrapMode: Text.Wrap
                }
            }
        }
    }
}
