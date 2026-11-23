import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

Card {
    id: chart
    objectName: "trendsChart"
    property var tip
    property var report: ({})
    property var metrics: []
    property var filter: ({ days: 28 })
    property string rangeKey: ""
    property string interval: "day"
    property string split: ""
    property var measures: ["minutes"]
    property bool relative: false
    property bool tableMode: false
    signal requested(var filter, var measures, string interval, string split)

    readonly property var choices: [
        { id: "minutes", name: qsTr("Minutes") }, { id: "sessions", name: qsTr("Sessions") },
        { id: "focus", name: qsTr("Focus") }, { id: "quality", name: qsTr("Quality") },
        { id: "distractions", name: qsTr("Distractions") },
        { id: "distractions_per_hour", name: qsTr("Distractions per hour") }
    ].concat((metrics || []).filter(m => !m.builtin && !["focus", "quality", "distractions"].includes(m.id)))
    readonly property var periods: report.periods || []
    readonly property var lineColors: ["#a0b7ce", "#d2a178", "#92b393", "#ba9ec5",
                                   "#c7bb83", "#80b7b4", "#cb9296", "#a5a3cd"]
    readonly property var lines: (report.series || []).map((s, index) => {
        let values = s.points.map(p => p.value).filter(v => v !== null && v !== undefined)
        return { series: s, index: index, low: values.length ? Math.min(...values) : 0,
                 high: values.length ? Math.max(...values) : 0 }
    })
    // A slot owns its colour until its line is turned off. Extra split keys
    // (including "Other") are available even when the initial eight are full.
    property var slots: []
    readonly property string roster: JSON.stringify([report.split, lines.map(l => [l.series.key, l.series.measure])])
    onRosterChanged: slots = lines.slice(0, lineColors.length).map((line, i) => i)
    readonly property var shown: lines.filter((line, i) => slots.includes(i))
    readonly property var groups: {
        let byUnit = []
        for (let line of shown) {
            let s = line.series
            let key = JSON.stringify([s.unit, s.scale_max])
            let group = byUnit.find(g => g.key === key)
            if (!group) {
                group = { key: key, title: s.scale_max ? qsTr("Rating (1–%1)").arg(s.scale_max)
                                                       : String(s.unit || qsTr("Count")), lines: [] }
                byUnit.push(group)
            }
            group.lines.push(line)
        }
        return byUnit
    }
    readonly property bool normalized: groups.length > 1 && relative
    readonly property var panels: normalized ? [{ title: qsTr("Relative to each line’s range"), lines: shown }] : groups
    readonly property var tableRows: tableMode ? periods.map((period, i) =>
        [period].concat(shown.map(line => valueText(line.series, i)))) : []
    implicitHeight: body.implicitHeight + 32

    function load(query) {
        let key = query.days ? String(query.days) : query.from + "/" + query.to
        if (key !== rangeKey) {
            rangeKey = key
            let days = query.days || (1 + (Date.parse(query.to) - Date.parse(query.from)) / 86400000)
            interval = days <= 31 ? "day" : "week"
        }
        filter = query
        let available = measures.filter(id => choices.some(c => c.id === id))
        measures = available.length ? available : ["minutes"]
        request()
    }
    function request() {
        requested(filter, measures, interval, split)
    }
    function toggleMeasure(id) {
        if (measures.includes(id)) {
            if (measures.length === 1) return
            measures = measures.filter(m => m !== id)
        } else {
            measures = measures.concat([id])
        }
        request()
    }
    function toggleLine(index) {
        let next = slots.slice()
        let slot = next.indexOf(index)
        if (slot >= 0) {
            next[slot] = -1
        } else {
            slot = next.indexOf(-1)
            if (slot < 0) return
            next[slot] = index
        }
        slots = next
    }
    function ink(line) {
        return lineColors[slots.indexOf(line.index)] || win.muted
    }
    function valueText(series, i) {
        let value = series.points[i].value
        return win.numberText(value) + (value !== null && value !== undefined && series.unit ? " " + series.unit : "")
    }
    function tooltip(i) {
        let date = win.shortDate(periods[i])
        let rows = [report.interval === "week" ? qsTr("Week of %1").arg(date) : date]
        for (let line of shown) {
            let s = line.series
            let count = s.points[i].sessions
            rows.push(s.label + "   " + valueText(s, i) + "   ·   " +
                      (count === 1 ? qsTr("1 session") : qsTr("%1 sessions").arg(count)))
        }
        return rows.join("\n")
    }

    component Caption: Label {
        textFormat: Text.PlainText
        color: chart.win.muted
        font.pixelSize: 12
        verticalAlignment: Text.AlignVCenter
    }
    component Chip: Button {
        id: chip
        property bool selected: false
        property color dot: "transparent"
        height: 28
        width: Math.min(implicitWidth, parent.width)
        leftPadding: 10
        rightPadding: 10
        topPadding: 0
        bottomPadding: 0
        topInset: 0
        bottomInset: 0
        contentItem: RowLayout {
            spacing: 6
            Rectangle {
                visible: chip.dot.a > 0
                implicitWidth: 8
                implicitHeight: 8
                radius: 4
                color: chip.dot
            }
            Text {
                Layout.fillWidth: true
                text: chip.text
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: chip.selected ? chart.win.selectedText : chart.win.muted
                font.pixelSize: 13
            }
        }
        background: Rectangle {
            radius: 6
            color: chip.selected ? chart.win.selectedFill : "transparent"
            border.color: chip.selected ? chart.win.selectedBorder : chart.win.border
        }
    }

    ColumnLayout {
        id: body
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 16
        spacing: 10
        RowLayout {
            Layout.fillWidth: true
            spacing: 12
            Label {
                text: qsTr("Trends")
                color: chart.win.text
                font.weight: Font.Medium
                font.pixelSize: 17
            }
            ChartModeToggle {
                win: chart.win
                table: chart.tableMode
                onChosen: table => chart.tableMode = table
            }
            Item { Layout.fillWidth: true }
            Row {
                spacing: 4
                Repeater {
                    model: [{ id: "day", name: qsTr("Day") }, { id: "week", name: qsTr("Week") }]
                    Chip {
                        required property var modelData
                        width: implicitWidth
                        objectName: "trendsInterval_" + modelData.id
                        text: modelData.name
                        selected: chart.interval === modelData.id
                        onClicked: { chart.interval = modelData.id; chart.request() }
                    }
                }
            }
        }

        GridLayout {
            Layout.fillWidth: true
            columns: 2
            columnSpacing: 14
            rowSpacing: 6
            Caption { text: qsTr("Measures"); Layout.alignment: Qt.AlignTop; Layout.preferredHeight: 28 }
            Flow {
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                spacing: 5
                Repeater {
                    model: chart.choices
                    Chip {
                        required property var modelData
                        objectName: "trendsMeasure_" + modelData.id
                        text: modelData.name
                        selected: chart.measures.includes(modelData.id)
                        onClicked: chart.toggleMeasure(modelData.id)
                    }
                }
            }
            Caption { text: qsTr("Split by"); Layout.alignment: Qt.AlignTop; Layout.preferredHeight: 28 }
            Flow {
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                spacing: 5
                Repeater {
                    model: [{ id: "", name: qsTr("None") }, { id: "item", name: qsTr("Task") },
                            { id: "project", name: qsTr("Project") }, { id: "tag", name: qsTr("Tag") }]
                    Chip {
                        required property var modelData
                        objectName: "trendsSplit_" + modelData.id
                        text: modelData.name
                        selected: chart.split === modelData.id
                        onClicked: { chart.split = modelData.id; chart.request() }
                    }
                }
            }
        }
        Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: chart.win.border }
        Flow {
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            spacing: 6
            visible: chart.lines.length > 0
            Repeater {
                model: chart.lines
                Chip {
                    required property int index
                    required property var modelData
                    objectName: "trendsLine_" + index
                    selected: chart.slots.includes(index)
                    dot: selected ? chart.ink(modelData) : chart.win.muted
                    text: modelData.series.label
                    enabled: selected || chart.shown.length < chart.lineColors.length
                    onClicked: chart.toggleLine(index)
                }
            }
        }
        Caption {
            visible: chart.lines.length > chart.lineColors.length
            text: qsTr("%1 of %2 lines shown · Select up to 8").arg(chart.shown.length).arg(chart.lines.length)
        }
        Row {
            visible: !chart.tableMode && chart.groups.length > 1
            spacing: 4
            Repeater {
                model: [{ relative: false, name: qsTr("Split") }, { relative: true, name: qsTr("Relative") }]
                Chip {
                    required property var modelData
                    objectName: modelData.relative ? "trendsRelative" : "trendsSplitView"
                    width: implicitWidth
                    text: modelData.name
                    selected: chart.relative === modelData.relative
                    onClicked: chart.relative = modelData.relative
                }
            }
        }
        TrendsPlot {
            Layout.fillWidth: true
            visible: !chart.tableMode && chart.shown.length > 0 && chart.periods.length > 0
            trends: chart
        }
        Flickable {
            Layout.fillWidth: true
            Layout.preferredHeight: 216
            visible: chart.tableMode && chart.shown.length > 0
            clip: true
            contentWidth: Math.max(width, 132 + chart.shown.length * 132)
            contentHeight: height
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.horizontal: ScrollBar {}
            ChartTable {
                width: parent.width
                height: parent.height - 12
                win: chart.win
                columns: [chart.interval === "day" ? qsTr("Day") : qsTr("Week of")].concat(chart.shown.map(l => l.series.label))
                rows: chart.tableRows
            }
        }
        Caption {
            visible: chart.shown.length === 0 || chart.periods.length === 0
            text: chart.lines.length ? qsTr("Select a line to display") : qsTr("No series for this selection")
        }
    }
}
