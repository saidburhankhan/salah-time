import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The Charts tab: KPI tiles, minutes per day against the plan, one line per
// metric, and project, tag and block breakdowns of the filtered sessions.
ColumnLayout {
    id: charts
    property var win
    property alias trends: trendsCard
    property var metrics
    property var tip
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
    spacing: 16

    function sessionsText(count) {
        return count === 1 ? qsTr("1 session") : qsTr("%1 sessions").arg(count || 0)
    }
    function metricText(metrics) {
        var parts = []
        for (var key in (metrics || {})) {
            var metric = win.metricById(key)
            var name = metric ? String(metric.name) : key
            parts.push(name + " " + win.numberText(metrics[key]) + (metric && metric.unit ? " " + metric.unit : ""))
        }
        return parts.join("  ·  ")
    }
    function projectRows() {
        var rows = stats.projects || []
        var total = Math.max(1, Number(totals.minutes || 0))
        return rows.map(function(p) {
            return { name: p.name, color: p.color, value: p.minutes, total: total,
                     detail: win.formatMinutes(p.minutes) + "  ·  " + charts.sessionsText(p.sessions),
                     extra: charts.metricText(p.metrics) }
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
                     extra: b.avg_focus === null || b.avg_focus === undefined ? "" : qsTr("Focus %1").arg(win.numberText(b.avg_focus)) }
        })
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 12
        SummaryTile {
            win: charts.win
            label: qsTr("Time tracked")
            value: charts.win.formatMinutes(charts.totals.minutes || 0)
            detail: charts.sessionsText(charts.totals.sessions)
        }
        SummaryTile {
            win: charts.win
            label: qsTr("Active days")
            value: String(charts.totals.active_days || 0) + " / " + charts.days.length
            detail: qsTr("days with a session")
        }
        SummaryTile {
            win: charts.win
            label: qsTr("Average focus")
            value: charts.win.numberText(charts.totals.avg_focus)
            detail: qsTr("quality %1").arg(charts.win.numberText(charts.totals.avg_quality))
        }
        SummaryTile {
            win: charts.win
            label: qsTr("Plan fulfilment")
            value: charts.fulfilment < 0 ? "—" : Math.round(charts.fulfilment * 100) + "%"
            detail: charts.plannedDays === 1 ? qsTr("over 1 planned day") : qsTr("over %1 planned days").arg(charts.plannedDays)
        }
    }

    TrendsChart {
        id: trendsCard
        Layout.fillWidth: true
        win: charts.win
        tip: charts.tip
        metrics: charts.metrics
        report: backend.trends
        onRequested: (filter, measures, interval, split) => backend.loadTrends(filter, measures, interval, split)
    }

    DayColumnChart {
        Layout.fillWidth: true
        win: charts.win
        tip: charts.tip
        title: qsTr("Minutes per day")
        subtitle: qsTr("capacity %1 a day").arg(charts.win.formatMinutes(Math.round(charts.capacityPerDay)))
        days: charts.days
    }

    GridLayout {
        visible: charts.metricSeries.length > 0
        Layout.fillWidth: true
        columns: 2
        columnSpacing: 12
        rowSpacing: 12
        Repeater {
            model: charts.metricSeries
            LineChart {
                required property var modelData
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                win: charts.win
                series: modelData
                ceiling: {
                    var metric = charts.win.metricById(modelData.metric_id)
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
            win: charts.win
            title: qsTr("Projects")
            rows: charts.projectRows()
        }
        BreakdownCard {
            Layout.preferredWidth: 1
            win: charts.win
            title: qsTr("Tags")
            empty: qsTr("No tagged time")
            rows: charts.tagRows()
        }
    }

    BreakdownCard {
        win: charts.win
        title: qsTr("Time per block")
        rows: charts.blockRows()
    }
}
