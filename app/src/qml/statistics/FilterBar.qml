import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// The one filter bar of the Statistics page, shared by its Charts, Table and
// Notes tabs: range (7/28/90 days or Custom), Tags, Projects (narrowed by the
// tag), Tasks (narrowed by the project) and Metrics (a duration range and a
// min–max range per metric, shown as removable chips). Every field is
// optional. `edited()` fires after any change; `query()` is the request filter.
ColumnLayout {
    id: bar
    property var win
    // 7, 28 or 90 days, or 0 for the custom from/to range.
    property int range: 28
    property string customFrom: ""
    property string customTo: ""
    // The last day of the current range as the server reported it; the
    // default end of a new Custom range.
    property string rangeTo: ""
    property string tagId: ""
    property string projectId: ""
    property string itemId: ""
    // Duration bounds in minutes, -1 when unset.
    property int minutesMin: -1
    property int minutesMax: -1
    // {metric_id: {min, max}}; a missing bound is undefined.
    property var valueRanges: ({})
    // The metric range editor: "" closed, "minutes" or a metric id.
    property string editing: ""
    readonly property int maxTasks: 24
    readonly property var projectChoices: (backend.projects || []).filter(function(p) {
        return !p.archived && (tagId === "" || (p.tag_ids || []).indexOf(tagId) >= 0)
    })
    readonly property var taskChoices: {
        var allowed = {}
        for (var i = 0; i < projectChoices.length; i++)
            allowed[String(projectChoices[i].id)] = true
        return (backend.trackingItems || []).filter(function(item) {
            var project = String(item.project_id || "")
            if (projectId !== "")
                return project === projectId
            return tagId === "" || allowed[project] === true
        })
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
    // kind: "" (clear everything), "tag", "project" or "item". Choosing a
    // wider filter drops a narrower one that no longer fits.
    function setFilter(kind, id) {
        if (kind === "tag") {
            tagId = id
            var fits = projectChoices.some(function(p) { return String(p.id) === projectId })
            if (projectId !== "" && !fits) {
                projectId = ""
                itemId = ""
            } else if (itemId !== "" && !taskChoices.some(function(t) { return String(t.id) === itemId })) {
                itemId = ""
            }
        } else if (kind === "project") {
            projectId = id
            itemId = ""
        } else if (kind === "item") {
            itemId = id
        } else {
            tagId = ""
            projectId = ""
            itemId = ""
        }
        edited()
    }
    function setRange(days) {
        range = days
        edited()
    }
    function setCustom(from, to) {
        customFrom = from
        customTo = to
        range = 0
        edited()
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
        if (tagId !== "") {
            var tag = win.tagById(tagId)
            if (tag)
                parts.push("# " + String(tag.name))
        }
        if (projectId !== "")
            parts.push(win.projectName(projectId))
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
        Layout.fillWidth: true
        spacing: 8
        RowTitle {
            text: qsTr("RANGE")
        }
        Repeater {
            model: [7, 28, 90]
            ChoiceChip {
                required property int modelData
                win: bar.win
                text: qsTr("%1 days").arg(modelData)
                selected: bar.range === modelData
                onClicked: bar.setRange(modelData)
            }
        }
        ChoiceChip {
            objectName: "statsCustomChip"
            win: bar.win
            text: qsTr("Custom")
            selected: bar.range === 0
            onClicked: bar.startCustom()
        }
        TextField {
            visible: bar.range === 0
            Layout.preferredWidth: 130
            text: bar.customFrom
            placeholderText: "YYYY-MM-DD"
            font.family: bar.win.mono
            onEditingFinished: if (bar.validDate(text) && text !== bar.customFrom) bar.setCustom(text, bar.customTo)
        }
        Label {
            visible: bar.range === 0
            text: "–"
            color: bar.win.muted
        }
        TextField {
            visible: bar.range === 0
            Layout.preferredWidth: 130
            text: bar.customTo
            placeholderText: "YYYY-MM-DD"
            font.family: bar.win.mono
            onEditingFinished: if (bar.validDate(text) && text !== bar.customTo) bar.setCustom(bar.customFrom, text)
        }
        Item {
            Layout.fillWidth: true
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
            Repeater {
                model: backend.tags || []
                ChoiceChip {
                    required property var modelData
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
        visible: bar.taskChoices.length > 0
        Layout.fillWidth: true
        spacing: 8
        RowTitle {
            text: qsTr("TASKS")
        }
        Flow {
            objectName: "statsTaskList"
            Layout.fillWidth: true
            spacing: 8
            ChoiceChip {
                win: bar.win
                text: qsTr("All")
                selected: bar.itemId === ""
                onClicked: bar.setFilter("item", "")
            }
            Repeater {
                model: bar.taskChoices.slice(0, bar.maxTasks)
                ChoiceChip {
                    required property var modelData
                    win: bar.win
                    text: String(modelData.title || "")
                    selected: bar.itemId === String(modelData.id)
                    onClicked: bar.setFilter("item", String(modelData.id))
                }
            }
            Label {
                visible: bar.taskChoices.length > bar.maxTasks
                height: 40
                verticalAlignment: Text.AlignVCenter
                text: qsTr("+%1 more, pick a project to narrow").arg(bar.taskChoices.length - bar.maxTasks)
                color: bar.win.muted
            }
        }
    }

    RowLayout {
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
        visible: bar.editing !== ""
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
