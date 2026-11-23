import QtQuick
import QtQuick.Controls

// One metric over the range. Averages (scales) are a line through the days
// that have a value, with gaps where a day has none; sums (counts, checks)
// are a column per day.
Rectangle {
    id: chart
    property var win
    // stats.series row: {metric_id, name, unit, rollup, points: [{day, value|null}]}
    property var series: ({})
    // The y axis top: a scale's maximum, otherwise the data's.
    property real ceiling: 0
    readonly property var points: series.points || []
    readonly property var present: points.filter(function(p) { return p.value !== null && p.value !== undefined })
    readonly property real yTop: {
        if (ceiling > 0)
            return ceiling
        var top = 1
        for (var i = 0; i < present.length; i++)
            top = Math.max(top, Number(present[i].value))
        return top
    }
    readonly property real summary: {
        if (present.length === 0)
            return 0
        var sum = 0
        for (var i = 0; i < present.length; i++)
            sum += Number(present[i].value)
        return series.rollup === "avg" ? sum / present.length : sum
    }
    property bool tableMode: false
    // Draw a sum as a line over the days instead of a column per day.
    property bool asLine: false
    readonly property var tableRows: points.slice().reverse().map(function(p) {
        var d = Date.fromLocaleString(Qt.locale(), String(p.day), "yyyy-MM-dd")
        return [Qt.locale("en_US").toString(d, "ddd d MMM"),
                p.value === null || p.value === undefined ? "—" : format(p.value)]
    })
    height: 190
    radius: theme.radius
    color: win.panel
    border.color: win.border

    function format(value) {
        var rounded = Math.round(Number(value) * 10) / 10
        return String(rounded) + (series.unit ? " " + series.unit : "")
    }

    onPointsChanged: canvas.requestPaint()
    onWidthChanged: canvas.requestPaint()
    onYTopChanged: canvas.requestPaint()
    onAsLineChanged: canvas.requestPaint()

    Row {
        x: 18
        y: 11
        spacing: 12
        Label {
            anchors.verticalCenter: parent.verticalCenter
            text: String(chart.series.name || "")
            textFormat: Text.PlainText
            color: chart.win.text
            font.pixelSize: 15
            font.weight: Font.Medium
        }
        ChartModeToggle {
            anchors.verticalCenter: parent.verticalCenter
            win: chart.win
            table: chart.tableMode
            onChosen: function(table) { chart.tableMode = table }
        }
    }
    ChartTable {
        visible: chart.tableMode
        anchors.fill: parent
        anchors.leftMargin: 18
        anchors.rightMargin: 18
        anchors.topMargin: 46
        anchors.bottomMargin: 10
        win: chart.win
        columns: [qsTr("DAY"), String(chart.series.name || "").toUpperCase()]
        rows: chart.tableRows
    }
    Label {
        anchors.right: parent.right
        anchors.rightMargin: 18
        y: 15
        text: chart.present.length === 0 ? qsTr("no values")
            : (chart.series.rollup === "avg" ? qsTr("avg %1") : qsTr("total %1")).arg(chart.format(chart.summary))
        textFormat: Text.PlainText
        color: chart.present.length === 0 ? chart.win.muted : theme.accent
        font.family: chart.win.mono
        font.pixelSize: 13
    }
    Label {
        visible: !chart.tableMode
        x: 12
        y: 44
        width: 30
        horizontalAlignment: Text.AlignRight
        text: chart.format(chart.yTop).split(" ")[0]
        color: chart.win.muted
        font.family: chart.win.mono
        font.pixelSize: 10
    }
    Label {
        visible: !chart.tableMode
        x: 12
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 18
        width: 30
        horizontalAlignment: Text.AlignRight
        text: "0"
        color: chart.win.muted
        font.family: chart.win.mono
        font.pixelSize: 10
    }
    Canvas {
        id: canvas
        visible: !chart.tableMode
        anchors.fill: parent
        anchors.leftMargin: 50
        anchors.rightMargin: 18
        anchors.topMargin: 50
        anchors.bottomMargin: 24
        onPaint: {
            var ctx = getContext("2d")
            ctx.reset()
            var n = chart.points.length
            if (n === 0)
                return
            ctx.strokeStyle = "#302e29"
            ctx.lineWidth = 1
            for (var g = 0; g <= 2; g++) {
                var gy = Math.round(g * (height - 1) / 2) + 0.5
                ctx.beginPath()
                ctx.moveTo(0, gy)
                ctx.lineTo(width, gy)
                ctx.stroke()
            }
            function xAt(i) { return n === 1 ? width / 2 : i * width / (n - 1) }
            function yAt(v) { return height - 3 - (height - 6) * Math.min(1, Number(v) / chart.yTop) }
            if (chart.series.rollup !== "avg" && !chart.asLine) {
                var slot = width / n
                var barWidth = Math.max(2, Math.min(24, slot - 3))
                ctx.fillStyle = String(theme.accent)
                for (var b = 0; b < n; b++) {
                    var amount = chart.points[b].value
                    if (amount === null || amount === undefined || Number(amount) <= 0)
                        continue
                    var barTop = yAt(amount)
                    ctx.fillRect(b * slot + (slot - barWidth) / 2, barTop, barWidth, height - barTop)
                }
                return
            }
            ctx.strokeStyle = String(theme.accent)
            ctx.lineWidth = 2
            ctx.beginPath()
            var drawing = false
            for (var i = 0; i < n; i++) {
                var v = chart.points[i].value
                if (v === null || v === undefined) {
                    drawing = false
                    continue
                }
                if (!drawing)
                    ctx.moveTo(xAt(i), yAt(v))
                else
                    ctx.lineTo(xAt(i), yAt(v))
                drawing = true
            }
            ctx.stroke()
            ctx.fillStyle = String(theme.accent)
            for (var j = 0; j < n; j++) {
                var value = chart.points[j].value
                if (value === null || value === undefined)
                    continue
                ctx.beginPath()
                ctx.arc(xAt(j), yAt(value), 2.5, 0, 2 * Math.PI)
                ctx.fill()
            }
        }
    }
}
