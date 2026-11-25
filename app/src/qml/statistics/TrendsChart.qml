import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "AxisScale.js" as AxisScale

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
    onRosterChanged: {
        slots = lines.slice(0, lineColors.length).map((line, i) => i)
        clearHighlight()
    }
    readonly property var shown: lines.filter(line => slots.includes(line.index))
    readonly property var axes: {
        const result = []
        for (const line of shown) {
            const s = line.series
            let axis = result.find(a => a.measure === s.measure)
            if (!axis) {
                const choice = choices.find(c => c.id === s.measure)
                axis = { measure: s.measure, name: choice ? choice.name : (!report.split && s.label || s.measure || s.name || ""),
                         unit: s.unit || "", low: s.scale_max ? 1 : 0, high: 1, lines: [] }
                result.push(axis)
            }
            axis.high = s.scale_max || Math.max(axis.high, AxisScale.niceMax(line.high))
            axis.lines.push(line)
        }
        return result
    }
    readonly property int extraAxes: Math.max(0, axes.length - 4)
    readonly property bool normalized: extraAxes > 0 || (axes.length > 1 && relative)
    property var pointerItem: null
    property string pointed: ""
    property string keyboardLine: ""
    property string pinned: ""
    readonly property string highlight: pinned || pointed || keyboardLine
    function clearHighlight() {
        pinned = ""
        pointerItem = null
        pointed = ""
        keyboardLine = ""
    }
    function pin(key) {
        if (pinned === key) clearHighlight()
        else pinned = key
    }
    function pointAt(item, key) {
        pointerItem = item
        pointed = key
    }
    function leave(item) {
        if (pointerItem === item) pointAt(null, "")
    }
    function lineKey(line) { return "line:" + line.index }
    function axisKey(axis) {
        return axis.lines.length === 1 ? lineKey(axis.lines[0]) : "axis:" + axis.measure
    }
    function highlighted(line) {
        return highlight === "" || highlight === lineKey(line) || highlight === "axis:" + line.series.measure
    }
    onTableModeChanged: clearHighlight()
    onVisibleChanged: if (!visible) clearHighlight()
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
        clearHighlight()
        slots = next
    }
    function ink(line) {
        return lineColors[slots.indexOf(line.index)] || win.muted
    }
    function valueText(series, i) {
        let value = series.points[i].value
        if (value === null || value === undefined) return "—"
        const unit = series.measure === "sessions" && value === 1 ? qsTr("session") : series.unit
        const suffix = unit ? " " + unit : (series.scale_max ? "/" + series.scale_max : "")
        return win.numberText(value) + suffix
    }
    function tooltip(i) {
        let date = win.shortDate(periods[i])
        let rows = [report.interval === "week" ? qsTr("Week of %1").arg(date) : date]
        const names = shown.map(l => l.series.label)
        const values = shown.map(l => valueText(l.series, i))
        const nameWidth = Math.max(0, ...names.map(s => s.length))
        const valueWidth = Math.max(0, ...values.map(s => s.length))
        for (let j = 0; j < shown.length; ++j) {
            const count = shown[j].series.points[i].sessions
            rows.push(names[j].padEnd(nameWidth) + "   " + values[j].padStart(valueWidth) + "   ·   " +
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
            border.color: chip.visualFocus ? chart.win.selectedText : chip.selected ? chart.win.selectedBorder : chart.win.border
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
                            { id: "project", name: qsTr("Project") }, { id: "epic", name: qsTr("Epic") },
                            { id: "tag", name: qsTr("Tag") }]
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
                Row {
                    id: legend
                    required property int index
                    required property var modelData
                    readonly property bool shown: chart.slots.includes(index)
                    readonly property string key: chart.lineKey(modelData)
                    spacing: 2
                    opacity: !shown || chart.highlighted(modelData) ? 1 : 0.35
                    Chip {
                        id: lineChip
                        objectName: "trendsLine_" + legend.index
                        width: Math.min(implicitWidth, body.width - (hideChip.visible ? hideChip.width + 2 : 0))
                        selected: legend.shown
                        dot: selected ? chart.ink(legend.modelData) : chart.win.muted
                        text: legend.modelData.series.label + (chart.pinned === legend.key ? qsTr(" · pinned") : "")
                        enabled: selected || chart.shown.length < chart.lineColors.length
                        hoverEnabled: true
                        focusPolicy: Qt.StrongFocus
                        Accessible.description: qsTr("Highlight or pin this series")
                        onHoveredChanged: {
                            if (hovered && selected) chart.pointAt(lineChip, legend.key)
                            else chart.leave(lineChip)
                        }
                        onVisualFocusChanged: {
                            if (visualFocus && selected) {
                                chart.pointAt(null, "")
                                chart.keyboardLine = legend.key
                            } else if (chart.keyboardLine === legend.key) chart.keyboardLine = ""
                        }
                        onClicked: {
                            if (legend.shown && !chart.tableMode) chart.pin(legend.key)
                            else chart.toggleLine(legend.index)
                        }
                    }
                    Chip {
                        id: hideChip
                        objectName: "trendsHide_" + legend.index
                        visible: legend.shown && !chart.tableMode && (!!chart.report.split || chart.lines.length > chart.lineColors.length)
                        width: 26
                        leftPadding: 7
                        rightPadding: 7
                        text: "−"
                        Accessible.name: qsTr("Hide %1").arg(legend.modelData.series.label)
                        onClicked: chart.toggleLine(legend.index)
                    }
                }
            }
        }
        Caption {
            visible: chart.lines.length > chart.lineColors.length
            text: qsTr("%1 of %2 lines shown · Select up to 8").arg(chart.shown.length).arg(chart.lines.length)
        }
        Flow {
            Layout.fillWidth: true
            visible: !chart.tableMode && chart.axes.length > 1
            spacing: 5
            Chip {
                objectName: "trendsAxesView"
                width: implicitWidth
                text: qsTr("Measure axes")
                enabled: chart.extraAxes === 0
                opacity: enabled ? 1 : 0.45
                selected: !chart.normalized
                onClicked: chart.relative = false
            }
            Chip {
                objectName: "trendsRelative"
                width: implicitWidth
                text: qsTr("Normalised")
                selected: chart.normalized
                onClicked: chart.relative = true
            }
            Caption {
                visible: chart.extraAxes > 0
                height: 28
                leftPadding: 5
                text: qsTr("+%1 beyond the 4-axis limit").arg(chart.extraAxes)
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
