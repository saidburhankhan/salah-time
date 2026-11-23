import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../statistics"

// Statistics for the last 7, 28 or 90 days, for everything or one tag or
// project: KPI tiles, minutes per day against the plan, one line per metric,
// and project, tag and block breakdowns.
ScrollView {
    id: statsPage
    objectName: "statsScroll"
    property var win
    property var tip
    // 7, 28 or 90 days, or 0 for the custom from/to range.
    property int range: 28
    property string customFrom: ""
    property string customTo: ""
    property string tagId: ""
    property string projectId: ""
    property string itemId: ""
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
    readonly property var stats: backend.statsData
    readonly property var totals: stats.totals || {}
    readonly property var days: stats.days || []
    readonly property var metricSeries: (stats.series || []).filter(function(s) { return s.metric_id !== "minutes" })
    readonly property real fulfilment: {
        var sum = 0
        var count = 0
        for (var i = 0; i < days.length; i++) {
            if (days[i].fulfilment_ratio === null || days[i].fulfilment_ratio === undefined)
                continue
            sum += Number(days[i].fulfilment_ratio)
            count++
        }
        return count > 0 ? sum / count : -1
    }
    readonly property int plannedDays: days.filter(function(d) { return Number(d.planned_minutes || 0) > 0 }).length
    readonly property real capacityPerDay: {
        if (days.length === 0)
            return 0
        var sum = 0
        for (var i = 0; i < days.length; i++)
            sum += Number(days[i].capacity_minutes || 0)
        return sum / days.length
    }
    clip: true
    contentWidth: availableWidth

    function open() {
        backend.loadTrackingItems()
        reload()
    }
    function validDate(text) {
        return /^\d{4}-\d{2}-\d{2}$/.test(text) && !isNaN(Date.parse(text))
    }
    function reload() {
        if (range === 0 && validDate(customFrom) && validDate(customTo) && customFrom <= customTo)
            backend.loadStatsRange(customFrom, customTo, tagId, projectId, itemId)
        else
            backend.loadStatsFiltered(range > 0 ? range : 28, tagId, projectId, itemId)
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
        reload()
    }
    function setRange(days) {
        range = days
        reload()
    }
    function setCustom(from, to) {
        customFrom = from
        customTo = to
        range = 0
        reload()
    }
    function score(value) {
        return value === null || value === undefined ? "—" : String(Math.round(Number(value) * 10) / 10)
    }
    function metricText(metrics) {
        var parts = []
        for (var key in (metrics || {})) {
            var metric = win.metricById(key)
            var name = metric ? String(metric.name) : key
            var value = Math.round(Number(metrics[key]) * 10) / 10
            parts.push(name + " " + value + (metric && metric.unit ? " " + metric.unit : ""))
        }
        return parts.join("  ·  ")
    }
    function projectRows() {
        var rows = stats.projects || []
        var total = Math.max(1, Number(totals.minutes || 0))
        return rows.map(function(p) {
            return { name: p.name, color: p.color, value: p.minutes, total: total,
                     detail: win.formatMinutes(p.minutes) + "  ·  " + (p.sessions === 1 ? qsTr("1 session") : qsTr("%1 sessions").arg(p.sessions)),
                     extra: statsPage.metricText(p.metrics) }
        })
    }
    function tagRows() {
        var rows = stats.tags || []
        var total = Math.max(1, Number(totals.minutes || 0))
        return rows.map(function(t) {
            return { name: t.name, color: t.color, value: t.minutes, total: total,
                     detail: win.formatMinutes(t.minutes) }
        })
    }
    function blockRows() {
        var rows = stats.blocks || []
        return rows.map(function(b) {
            var index = win.blockIds.indexOf(String(b.block))
            var share = b.capacity_minutes > 0 ? Math.round(100 * b.minutes / b.capacity_minutes) : 0
            return { name: index >= 0 ? win.blockNames[index] : b.block, color: theme.accent,
                     value: b.minutes, total: b.capacity_minutes,
                     detail: win.formatMinutes(b.minutes) + qsTr(" of ") + win.formatMinutes(b.capacity_minutes)
                         + "  ·  " + share + "%",
                     extra: b.avg_focus === null || b.avg_focus === undefined ? "" : qsTr("Focus %1").arg(statsPage.score(b.avg_focus)) }
        })
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

    ColumnLayout {
        width: statsPage.availableWidth
        spacing: 16

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Label {
                text: qsTr("RANGE")
                color: statsPage.win.muted
                font.family: statsPage.win.mono
                font.pixelSize: 11
                Layout.preferredWidth: 76
            }
            Repeater {
                model: [7, 28, 90]
                ChoiceChip {
                    required property int modelData
                    win: statsPage.win
                    text: qsTr("%1 days").arg(modelData)
                    selected: statsPage.range === modelData
                    onClicked: statsPage.setRange(modelData)
                }
            }
            ChoiceChip {
                objectName: "statsCustomChip"
                win: statsPage.win
                text: qsTr("Custom")
                selected: statsPage.range === 0
                onClicked: {
                    if (!statsPage.validDate(statsPage.customFrom) || !statsPage.validDate(statsPage.customTo)) {
                        var last = new Date(String(statsPage.stats.range ? statsPage.stats.range.to : Qt.formatDate(new Date(), "yyyy-MM-dd")) + "T12:00:00")
                        var first = new Date(last.getTime() - 29 * 86400000)
                        statsPage.customTo = Qt.formatDate(last, "yyyy-MM-dd")
                        statsPage.customFrom = Qt.formatDate(first, "yyyy-MM-dd")
                    }
                    statsPage.setCustom(statsPage.customFrom, statsPage.customTo)
                }
            }
            TextField {
                id: fromField
                visible: statsPage.range === 0
                Layout.preferredWidth: 130
                text: statsPage.customFrom
                placeholderText: "YYYY-MM-DD"
                font.family: statsPage.win.mono
                onEditingFinished: if (statsPage.validDate(text)) statsPage.setCustom(text, statsPage.customTo)
            }
            Label {
                visible: statsPage.range === 0
                text: "–"
                color: statsPage.win.muted
            }
            TextField {
                id: toField
                visible: statsPage.range === 0
                Layout.preferredWidth: 130
                text: statsPage.customTo
                placeholderText: "YYYY-MM-DD"
                font.family: statsPage.win.mono
                onEditingFinished: if (statsPage.validDate(text)) statsPage.setCustom(statsPage.customFrom, text)
            }
            Item { Layout.fillWidth: true }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Label {
                text: qsTr("TAGS")
                color: statsPage.win.muted
                font.family: statsPage.win.mono
                font.pixelSize: 11
                Layout.preferredWidth: 76
                Layout.alignment: Qt.AlignTop
                Layout.topMargin: 14
            }
            Flow {
                objectName: "statsTagList"
                Layout.fillWidth: true
                spacing: 8
                ChoiceChip {
                    win: statsPage.win
                    text: qsTr("All")
                    selected: statsPage.tagId === ""
                    onClicked: statsPage.setFilter("tag", "")
                }
                Repeater {
                    model: backend.tags || []
                    ChoiceChip {
                        required property var modelData
                        win: statsPage.win
                        text: String(modelData.name || "")
                        dotColor: String(modelData.color || "transparent")
                        selected: statsPage.tagId === String(modelData.id)
                        onClicked: statsPage.setFilter("tag", String(modelData.id))
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Label {
                text: qsTr("PROJECTS")
                color: statsPage.win.muted
                font.family: statsPage.win.mono
                font.pixelSize: 11
                Layout.preferredWidth: 76
                Layout.alignment: Qt.AlignTop
                Layout.topMargin: 14
            }
            Flow {
                objectName: "statsProjectList"
                Layout.fillWidth: true
                spacing: 8
                ChoiceChip {
                    win: statsPage.win
                    text: qsTr("All")
                    selected: statsPage.projectId === ""
                    onClicked: statsPage.setFilter("project", "")
                }
                Repeater {
                    model: statsPage.projectChoices
                    ChoiceChip {
                        required property var modelData
                        win: statsPage.win
                        text: String(modelData.name || "")
                        dotColor: String(modelData.color || "transparent")
                        selected: statsPage.projectId === String(modelData.id)
                        onClicked: statsPage.setFilter("project", String(modelData.id))
                    }
                }
            }
        }

        RowLayout {
            visible: statsPage.taskChoices.length > 0
            Layout.fillWidth: true
            spacing: 8
            Label {
                text: qsTr("TASKS")
                color: statsPage.win.muted
                font.family: statsPage.win.mono
                font.pixelSize: 11
                Layout.preferredWidth: 76
                Layout.alignment: Qt.AlignTop
                Layout.topMargin: 14
            }
            Flow {
                objectName: "statsTaskList"
                Layout.fillWidth: true
                spacing: 8
                ChoiceChip {
                    win: statsPage.win
                    text: qsTr("All")
                    selected: statsPage.itemId === ""
                    onClicked: statsPage.setFilter("item", "")
                }
                Repeater {
                    model: statsPage.taskChoices.slice(0, statsPage.maxTasks)
                    ChoiceChip {
                        required property var modelData
                        win: statsPage.win
                        text: String(modelData.title || "")
                        selected: statsPage.itemId === String(modelData.id)
                        onClicked: statsPage.setFilter("item", String(modelData.id))
                    }
                }
                Label {
                    visible: statsPage.taskChoices.length > statsPage.maxTasks
                    anchors.verticalCenter: undefined
                    height: 40
                    verticalAlignment: Text.AlignVCenter
                    text: qsTr("+%1 more, pick a project to narrow").arg(statsPage.taskChoices.length - statsPage.maxTasks)
                    color: statsPage.win.muted
                }
            }
        }

        Label {
            visible: !!statsPage.stats.range
            text: (statsPage.filterName() !== "" ? statsPage.filterName() + "  ·  " : "")
                + statsPage.win.shortDate(statsPage.stats.range ? statsPage.stats.range.from : "")
                + " – " + statsPage.win.shortDate(statsPage.stats.range ? statsPage.stats.range.to : "")
            textFormat: Text.PlainText
            color: statsPage.win.muted
            font.family: statsPage.win.mono
            font.pixelSize: 12
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 12
            SummaryTile {
                win: statsPage.win
                label: qsTr("Time tracked")
                value: statsPage.win.formatMinutes(statsPage.totals.minutes || 0)
                detail: (statsPage.totals.sessions === 1 ? qsTr("1 session") : qsTr("%1 sessions").arg(statsPage.totals.sessions || 0))
            }
            SummaryTile {
                win: statsPage.win
                label: qsTr("Active days")
                value: String(statsPage.totals.active_days || 0) + " / " + statsPage.days.length
                detail: qsTr("days with a session")
            }
            SummaryTile {
                win: statsPage.win
                label: qsTr("Average focus")
                value: statsPage.score(statsPage.totals.avg_focus)
                detail: qsTr("quality %1").arg(statsPage.score(statsPage.totals.avg_quality))
            }
            SummaryTile {
                win: statsPage.win
                label: qsTr("Plan fulfilment")
                value: statsPage.fulfilment < 0 ? "—" : Math.round(statsPage.fulfilment * 100) + "%"
                detail: statsPage.plannedDays === 1 ? qsTr("over 1 planned day") : qsTr("over %1 planned days").arg(statsPage.plannedDays)
            }
        }

        DayColumnChart {
            Layout.fillWidth: true
            win: statsPage.win
            tip: statsPage.tip
            title: qsTr("Minutes per day")
            subtitle: qsTr("capacity %1 a day").arg(statsPage.win.formatMinutes(Math.round(statsPage.capacityPerDay)))
            days: statsPage.days
        }

        GridLayout {
            visible: statsPage.metricSeries.length > 0
            Layout.fillWidth: true
            columns: 2
            columnSpacing: 12
            rowSpacing: 12
            Repeater {
                model: statsPage.metricSeries
                LineChart {
                    required property var modelData
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    win: statsPage.win
                    series: modelData
                    ceiling: {
                        var metric = statsPage.win.metricById(modelData.metric_id)
                        return metric && metric.kind === "scale" ? Number(metric.scale_max || 5)
                            : (metric && metric.kind === "check" ? 1 : 0)
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 12
            BreakdownCard {
                Layout.preferredWidth: 1
                win: statsPage.win
                title: qsTr("Projects")
                rows: statsPage.projectRows()
            }
            BreakdownCard {
                Layout.preferredWidth: 1
                win: statsPage.win
                title: qsTr("Tags")
                empty: qsTr("No tagged time")
                rows: statsPage.tagRows()
            }
        }

        BreakdownCard {
            win: statsPage.win
            title: qsTr("Time per block")
            rows: statsPage.blockRows()
        }

        Label {
            visible: backend.busy
            Layout.fillWidth: true
            text: qsTr("Loading statistics…")
            color: statsPage.win.muted
            horizontalAlignment: Text.AlignHCenter
        }
        Item {
            Layout.preferredHeight: 12
        }
    }
}
