import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Basic as Basic
import QtQuick.Layouts
import "../controls"

// The one filter bar of the Statistics page, shared by its Charts, Table and
// Notes tabs: Range, Projects, Epics, Tasks, Tags and Metrics. Duration and
// metric ranges are shown as removable chips. Every field is
// optional. `edited()` fires after any change; `query()` is the request filter.
ColumnLayout {
    id: bar
    property var win
    // 7, 28 or 90 days, or 0 for the custom from/to range.
    property bool showMetrics: true
    property int range: 28
    property string customFrom: ""
    property string customTo: ""
    // The last day of the current range as the server reported it; the
    // default end of a new Custom range.
    property string rangeTo: ""
    property string tagId: ""
    property string projectId: ""
    // An epic id, "none" (work outside any epic), or "" for all.
    property string epicId: ""
    property string itemId: ""
    // Duration bounds in minutes, -1 when unset.
    property int minutesMin: -1
    property int minutesMax: -1
    // {metric_id: {min, max}}; a missing bound is undefined.
    property var valueRanges: ({})
    // The metric range editor: "" closed, "minutes" or a metric id.
    property string editing: ""
    property string taskSearch: ""
    property string rangeError: ""
    readonly property var matchingTasks: taskChoices.filter(function(task) {
        return String(task.title || "").toLowerCase().includes(taskSearch.trim().toLowerCase())
    })
    readonly property var projectChoices: (backend.projects || []).filter(function(p) {
        return !p.archived
    })
    // The epics of the shown projects, narrowed by the chosen project.
    readonly property var epicChoices: {
        var allowed = {}
        for (var i = 0; i < projectChoices.length; i++)
            allowed[String(projectChoices[i].id)] = true
        return (backend.epics || []).filter(function(epic) {
            var project = String(epic.project_id)
            return allowed[project] === true && (projectId === "" || project === projectId)
        })
    }
    readonly property var scopedTasks: (backend.trackingItems || []).filter(function(item) {
        var project = String(item.project_id || "none")
        var epic = String(item.epic_id || "none")
        return (projectId === "" || project === projectId) && (epicId === "" || epic === epicId)
    })
    readonly property var taskChoices: scopedTasks.filter(function(item) {
        var ids = item.tag_ids || []
        return tagId === "" || (tagId === "none" ? ids.length === 0 : ids.indexOf(tagId) >= 0)
    })
    readonly property var tagChoices: {
        var tags = backend.tags || []
        if (projectId === "")
            return tags
        var ids = new Set()
        for (var task of scopedTasks)
            for (var id of task.tag_ids || [])
                ids.add(id)
        return tags.filter(function(tag) { return ids.has(tag.id) })
    }
    // Focus, quality, distractions and, with a project chosen, its own
    // metrics; otherwise every active metric.
    readonly property var metricChoices: {
        var list = []
        var seen = {}
        function add(metric) {
            if (!metric || metric.archived || seen[metric.id])
                return
            seen[metric.id] = true
            list.push(metric)
        }
        var builtins = ["focus", "quality", "distractions"]
        for (var i = 0; i < builtins.length; i++)
            add(win.metricById(builtins[i]) || (builtins[i] === "distractions" ? win.distractionMetric() : null))
        var project = win.projectById(projectId)
        if (project) {
            var links = project.metrics || []
            for (var j = 0; j < links.length; j++)
                add(links[j].metric || win.metricById(links[j].metric_id))
        } else {
            var all = backend.metrics || []
            for (var k = 0; k < all.length; k++)
                add(all[k])
        }
        return list
    }
    readonly property var activeChips: {
        var chips = []
        if (minutesMin >= 0 || minutesMax >= 0)
            chips.push({ key: "minutes", text: rangeText(qsTr("Duration"), minutesMin >= 0 ? minutesMin : undefined,
                                                         minutesMax >= 0 ? minutesMax : undefined, qsTr("min")) })
        for (var id in valueRanges) {
            var metric = win.metricById(id)
            var name = metric ? String(metric.name) : (id === "distractions" ? qsTr("Distractions") : id)
            chips.push({ key: id, text: rangeText(name, valueRanges[id].min, valueRanges[id].max, "") })
        }
        return chips
    }
    signal edited()
    spacing: 14

    function validDate(text) {
        return /^\d{4}-\d{2}-\d{2}$/.test(text) && !isNaN(Date.parse(text))
            && new Date(Date.parse(text)).toISOString().slice(0, 10) === text
    }
    function has(value) {
        return value !== undefined && value !== null && !isNaN(Number(value))
    }
    function rangeText(name, min, max, unit) {
        var suffix = unit ? " " + unit : ""
        if (has(min) && has(max))
            return Number(min) === Number(max) ? name + " = " + win.numberText(min) + suffix
                                               : name + " " + win.numberText(min) + "–" + win.numberText(max) + suffix
        if (has(min))
            return name + " ≥ " + win.numberText(min) + suffix
        return name + " ≤ " + win.numberText(max) + suffix
    }
    function query() {
        var q = {}
        if (range === 0 && validDate(customFrom) && validDate(customTo) && customFrom <= customTo) {
            q.from = customFrom
            q.to = customTo
        } else {
            q.days = range > 0 ? range : 28
        }
        if (tagId !== "")
            q.tag_id = tagId
        if (projectId !== "")
            q.project_id = projectId
        if (epicId !== "")
            q.epic_id = epicId
        if (itemId !== "")
            q.item_id = itemId
        if (minutesMin >= 0)
            q.minutes_min = minutesMin
        if (minutesMax >= 0)
            q.minutes_max = minutesMax
        var values = {}
        var any = false
        for (var id in valueRanges) {
            var bounds = {}
            if (has(valueRanges[id].min))
                bounds.min = Number(valueRanges[id].min)
            if (has(valueRanges[id].max))
                bounds.max = Number(valueRanges[id].max)
            values[id] = bounds
            any = true
        }
        if (any)
            q.values = values
        return q
    }
    function epicLabel(epic) {
        var name = String(epic.name || "")
        return projectId === "" ? win.projectName(epic.project_id) + " · " + name : name
    }
    // kind: "" (clear everything), "tag", "project", "epic" or "item".
    // A project, epic or tag change can make a selected task unavailable.
    function setFilter(kind, id) {
        if (kind === "tag") {
            tagId = id
            clearUnavailableTask()
        } else if (kind === "project") {
            projectId = id
            epicId = ""
            itemId = ""
        } else if (kind === "epic") {
            epicId = id
            clearUnavailableTask()
        } else if (kind === "item") {
            itemId = id
        } else {
            tagId = ""
            projectId = ""
            epicId = ""
            itemId = ""
        }
        edited()
    }
    function clearUnavailableTask() {
        if (itemId !== "" && !taskChoices.some(function(task) { return String(task.id) === itemId }))
            itemId = ""
    }
    function setRange(days) {
        rangeError = ""
        range = days
        edited()
    }
    function setCustom(from, to) {
        if (!validDate(from) || !validDate(to) || from > to || Date.parse(to) - Date.parse(from) >= 730 * 86400000) {
            rangeError = qsTr("Choose valid dates in order. The range can include up to 730 days.")
            return
        }
        rangeError = ""
        customFrom = from
        customTo = to
        range = 0
        edited()
    }
    function stepRange(direction) {
        var last = Date.parse(range === 0 ? customTo : backend.todayDay)
        if (isNaN(last)) return
        var day = 86400000
        var first = range === 0 ? Date.parse(customFrom) : last - (range - 1) * day
        var offset = direction * (last - first + day)
        setCustom(new Date(first + offset).toISOString().slice(0, 10),
                  new Date(last + offset).toISOString().slice(0, 10))
    }
    function startCustom() {
        if (!validDate(customFrom) || !validDate(customTo)) {
            var end = validDate(rangeTo) ? rangeTo : (backend.todayDay || Qt.formatDate(new Date(), "yyyy-MM-dd"))
            var last = new Date(end + "T12:00:00")
            var first = new Date(last.getTime() - 29 * 86400000)
            customTo = Qt.formatDate(last, "yyyy-MM-dd")
            customFrom = Qt.formatDate(first, "yyyy-MM-dd")
        }
        setCustom(customFrom, customTo)
    }
    // id: "minutes" or a metric id; min/max: numbers, or undefined/"" to
    // leave that side open. Both open removes the filter.
    function setMetricRange(id, min, max) {
        var low = min === "" ? undefined : min
        var high = max === "" ? undefined : max
        if (has(low) && has(high) && Number(low) > Number(high)) {
            var swap = low
            low = high
            high = swap
        }
        if (id === "minutes") {
            minutesMin = has(low) ? Math.max(0, Math.round(Number(low))) : -1
            minutesMax = has(high) ? Math.max(0, Math.round(Number(high))) : -1
        } else {
            var copy = {}
            for (var key in valueRanges)
                copy[key] = valueRanges[key]
            if (has(low) || has(high))
                copy[id] = { min: has(low) ? Number(low) : undefined, max: has(high) ? Number(high) : undefined }
            else
                delete copy[id]
            valueRanges = copy
        }
        editing = ""
        edited()
    }
    function removeChip(key) {
        setMetricRange(key, undefined, undefined)
    }
    function clearMetrics() {
        minutesMin = -1
        minutesMax = -1
        valueRanges = {}
        editing = ""
        edited()
    }
    function openEditor(id) {
        editing = id
        var low = id === "minutes" ? (minutesMin >= 0 ? minutesMin : undefined) : (valueRanges[id] || {}).min
        var high = id === "minutes" ? (minutesMax >= 0 ? minutesMax : undefined) : (valueRanges[id] || {}).max
        minField.text = has(low) ? String(low) : ""
        maxField.text = has(high) ? String(high) : ""
        minField.forceActiveFocus()
    }
    function editorHint() {
        if (editing === "minutes")
            return qsTr("minutes")
        var metric = win.metricById(editing) || (editing === "distractions" ? win.distractionMetric() : null)
        if (!metric)
            return ""
        if (metric.kind === "scale")
            return qsTr("scale 1–%1").arg(metric.scale_max || 5)
        if (metric.kind === "check")
            return qsTr("0 = no, 1 = yes")
        return String(metric.unit || "")
    }
    function filterName() {
        var parts = []
        if (tagId === "none") {
            parts.push(qsTr("No tag"))
        } else if (tagId !== "") {
            var tag = win.tagById(tagId)
            if (tag)
                parts.push("# " + String(tag.name))
        }
        if (projectId !== "")
            parts.push(projectId === "none" ? qsTr("No project") : win.projectName(projectId))
        if (epicId === "none")
            parts.push(qsTr("No epic"))
        else if (epicId !== "")
            parts.push(win.epicName(epicId))
        if (itemId !== "") {
            var task = (backend.trackingItems || []).filter(function(t) { return String(t.id) === itemId })[0]
            if (task)
                parts.push(String(task.title))
        }
        return parts.join(" / ")
    }

    component RowTitle: Label {
        color: bar.win.muted
        font.family: bar.win.mono
        font.pixelSize: 11
        Layout.preferredWidth: 76
        Layout.alignment: Qt.AlignTop
        Layout.topMargin: 14
    }

    RowLayout {
        objectName: "statsRangeList"
        Layout.fillWidth: true
        spacing: 8
        RowTitle {
            text: qsTr("RANGE")
        }
        Flow {
            Layout.fillWidth: true
            spacing: 8
            Row {
                spacing: 8
                height: bar.range === 0 ? customDates.implicitHeight : implicitHeight
                Repeater {
                    model: [7, 28, 90]
                    ChoiceChip {
                        required property int modelData
                        anchors.verticalCenter: parent.verticalCenter
                        win: bar.win
                        text: qsTr("%1 days").arg(modelData)
                        selected: bar.range === modelData
                        onClicked: bar.setRange(modelData)
                    }
                }
                ChoiceChip {
                    objectName: "statsCustomChip"
                    anchors.verticalCenter: parent.verticalCenter
                    win: bar.win
                    text: qsTr("Custom")
                    selected: bar.range === 0
                    onClicked: bar.startCustom()
                }
            }
            Row {
                id: customDates
                visible: bar.range === 0
                spacing: 8
                TextField {
                    id: fromField
                    width: 130
                    text: bar.customFrom
                    placeholderText: "YYYY-MM-DD"
                    Accessible.name: qsTr("Start date")
                    font.family: bar.win.mono
                    onEditingFinished: if (text !== bar.customFrom || bar.rangeError) bar.setCustom(text, toField.text)
                }
                Label {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "–"
                    color: bar.win.muted
                }
                TextField {
                    id: toField
                    objectName: "filterEndDate"
                    width: 130
                    text: bar.customTo
                    placeholderText: "YYYY-MM-DD"
                    Accessible.name: qsTr("End date")
                    font.family: bar.win.mono
                    onEditingFinished: if (text !== bar.customTo || bar.rangeError) bar.setCustom(fromField.text, text)
                }
            }
        }
    }

    Label {
        visible: bar.range === 0 && bar.rangeError !== ""
        text: bar.rangeError
        textFormat: Text.PlainText
        color: "#df9986"
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        RowTitle {
            text: qsTr("PROJECTS")
        }
        Flow {
            objectName: "statsProjectList"
            Layout.fillWidth: true
            spacing: 8
            ChoiceChip {
                win: bar.win
                text: qsTr("All")
                selected: bar.projectId === ""
                onClicked: bar.setFilter("project", "")
            }
            ChoiceChip {
                objectName: "statsProject_none"
                win: bar.win
                text: qsTr("No project")
                selected: bar.projectId === "none"
                onClicked: bar.setFilter("project", "none")
            }
            Repeater {
                model: bar.projectChoices
                ChoiceChip {
                    required property var modelData
                    win: bar.win
                    text: String(modelData.name || "")
                    dotColor: String(modelData.color || "transparent")
                    selected: bar.projectId === String(modelData.id)
                    onClicked: bar.setFilter("project", String(modelData.id))
                }
            }
        }
    }

    RowLayout {
        visible: bar.epicChoices.length > 0
        Layout.fillWidth: true
        spacing: 8
        RowTitle {
            text: qsTr("EPICS")
        }
        Flow {
            objectName: "statsEpicList"
            Layout.fillWidth: true
            spacing: 8
            ChoiceChip {
                win: bar.win
                text: qsTr("All")
                selected: bar.epicId === ""
                onClicked: bar.setFilter("epic", "")
            }
            Repeater {
                model: bar.epicChoices
                ChoiceChip {
                    required property var modelData
                    objectName: "statsEpic_" + String(modelData.id)
                    win: bar.win
                    text: bar.epicLabel(modelData)
                    selected: bar.epicId === String(modelData.id)
                    onClicked: bar.setFilter("epic", String(modelData.id))
                }
            }
            ChoiceChip {
                objectName: "statsEpic_none"
                win: bar.win
                text: qsTr("No epic")
                selected: bar.epicId === "none"
                onClicked: bar.setFilter("epic", "none")
            }
        }
    }

    RowLayout {
        visible: bar.scopedTasks.length > 0
        Layout.fillWidth: true
        spacing: 8
        RowTitle {
            text: qsTr("TASKS")
        }
        ColumnLayout {
            Layout.fillWidth: true
            Basic.TextField {
                id: taskSearchField
                objectName: "statsTaskSearch"
                Layout.preferredWidth: 240
                Layout.preferredHeight: allTasksChip.implicitHeight
                topInset: allTasksChip.topInset
                bottomInset: allTasksChip.bottomInset
                placeholderText: qsTr("Search tasks")
                text: bar.taskSearch
                font.pixelSize: 14
                leftPadding: 14
                rightPadding: 14
                color: bar.win.text
                placeholderTextColor: bar.win.muted
                selectionColor: bar.win.selectedFillPressed
                selectedTextColor: bar.win.selectedText
                background: Rectangle {
                    radius: Math.max(6, theme.radius)
                    color: bar.win.panel
                    border.color: taskSearchField.activeFocus ? bar.win.selectedBorder : bar.win.border
                }
                onTextEdited: bar.taskSearch = text
            }
            ScrollView {
                id: taskScroll
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(taskChips.implicitHeight, 136)
                contentWidth: availableWidth
                clip: true
                Flow {
                    id: taskChips
                    objectName: "statsTaskList"
                    width: taskScroll.availableWidth
                    spacing: 8
                    ChoiceChip {
                        id: allTasksChip
                        objectName: "statsTaskAll"
                        win: bar.win
                        text: qsTr("All")
                        selected: bar.itemId === ""
                        onClicked: bar.setFilter("item", "")
                    }
                    Repeater {
                        model: bar.matchingTasks
                        ChoiceChip {
                            required property var modelData
                            win: bar.win
                            objectName: "statsTask_" + modelData.id
                            text: String(modelData.title || "")
                            selected: bar.itemId === String(modelData.id)
                            onClicked: bar.setFilter("item", String(modelData.id))
                        }
                    }
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        RowTitle {
            text: qsTr("TAGS")
        }
        Flow {
            objectName: "statsTagList"
            Layout.fillWidth: true
            spacing: 8
            ChoiceChip {
                win: bar.win
                text: qsTr("All")
                selected: bar.tagId === ""
                onClicked: bar.setFilter("tag", "")
            }
            ChoiceChip {
                objectName: "statsTag_none"
                win: bar.win
                text: qsTr("No tag")
                selected: bar.tagId === "none"
                onClicked: bar.setFilter("tag", "none")
            }
            Repeater {
                model: bar.tagChoices
                ChoiceChip {
                    required property var modelData
                    objectName: "statsTag_" + modelData.id
                    win: bar.win
                    text: String(modelData.name || "")
                    dotColor: String(modelData.color || "transparent")
                    selected: bar.tagId === String(modelData.id)
                    onClicked: bar.setFilter("tag", String(modelData.id))
                }
            }
        }
    }

    RowLayout {
        visible: bar.showMetrics
        Layout.fillWidth: true
        spacing: 8
        RowTitle {
            text: qsTr("METRICS")
        }
        Flow {
            objectName: "statsMetricList"
            Layout.fillWidth: true
            spacing: 8
            Repeater {
                model: bar.activeChips
                FilterChip {
                    required property var modelData
                    objectName: "metricChip_" + modelData.key
                    win: bar.win
                    text: String(modelData.text)
                    onEdit: bar.openEditor(String(modelData.key))
                    onRemove: bar.removeChip(String(modelData.key))
                }
            }
            ChoiceChip {
                objectName: "metricAddChip"
                win: bar.win
                text: qsTr("+ Filter")
                selected: bar.editing !== ""
                onClicked: {
                    if (bar.editing !== "")
                        bar.editing = ""
                    else
                        bar.openEditor("minutes")
                }
            }
            ChoiceChip {
                objectName: "metricClearChip"
                visible: bar.activeChips.length > 0
                win: bar.win
                text: qsTr("Clear")
                onClicked: bar.clearMetrics()
            }
        }
    }

    // The range editor: pick Duration or a metric, then its min and max.
    Rectangle {
        objectName: "metricEditor"
        visible: bar.showMetrics && bar.editing !== ""
        Layout.fillWidth: true
        Layout.leftMargin: 84
        implicitHeight: editor.implicitHeight + 28
        radius: Math.max(6, theme.radius)
        color: bar.win.card
        border.color: bar.win.border
        ColumnLayout {
            id: editor
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 14
            spacing: 12
            Flow {
                Layout.fillWidth: true
                spacing: 8
                ChoiceChip {
                    win: bar.win
                    text: qsTr("Duration")
                    selected: bar.editing === "minutes"
                    onClicked: bar.openEditor("minutes")
                }
                Repeater {
                    model: bar.metricChoices
                    ChoiceChip {
                        required property var modelData
                        win: bar.win
                        text: String(modelData.name || modelData.id)
                        selected: bar.editing === String(modelData.id)
                        onClicked: bar.openEditor(String(modelData.id))
                    }
                }
            }
            RowLayout {
                spacing: 8
                TextField {
                    id: minField
                    objectName: "metricMinField"
                    Layout.preferredWidth: 96
                    placeholderText: qsTr("min")
                    font.family: bar.win.mono
                    inputMethodHints: Qt.ImhFormattedNumbersOnly
                    validator: DoubleValidator { bottom: 0; decimals: 2 }
                    onAccepted: bar.setMetricRange(bar.editing, text, maxField.text)
                }
                Label {
                    text: "–"
                    color: bar.win.muted
                }
                TextField {
                    id: maxField
                    objectName: "metricMaxField"
                    Layout.preferredWidth: 96
                    placeholderText: qsTr("max")
                    font.family: bar.win.mono
                    inputMethodHints: Qt.ImhFormattedNumbersOnly
                    validator: DoubleValidator { bottom: 0; decimals: 2 }
                    onAccepted: bar.setMetricRange(bar.editing, minField.text, text)
                }
                Label {
                    text: bar.editorHint()
                    textFormat: Text.PlainText
                    color: bar.win.muted
                    font.pixelSize: 13
                    Layout.leftMargin: 4
                }
                Item {
                    Layout.fillWidth: true
                }
                AppButton {
                    win: bar.win
                    text: qsTr("Cancel")
                    onClicked: bar.editing = ""
                }
                AppButton {
                    objectName: "metricApplyButton"
                    win: bar.win
                    primary: true
                    text: qsTr("Apply")
                    onClicked: bar.setMetricRange(bar.editing, minField.text, maxField.text)
                }
            }
        }
    }
}
