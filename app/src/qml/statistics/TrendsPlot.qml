import QtQuick
import QtQuick.Controls

Item {
    id: plot
    objectName: "trendsPlot"
    required property var trends
    readonly property var periods: trends.periods
    readonly property var axes: trends.normalized
        ? [{ measure: "%", name: qsTr("Range"), unit: "", low: 0, high: 100, lines: [] }]
        : trends.axes
    readonly property real columnWidth: Math.min(80, width * 0.47 / Math.max(1, axes.length))
    readonly property real leftGutter: axes.length * columnWidth + 10
    readonly property real plotWidth: Math.max(1, width - leftGutter - 16)
    readonly property int labelCount: Math.min(periods.length, Math.max(2, Math.min(7, Math.floor(plotWidth / 95))))
    readonly property real topInset: headings.height + 12
    readonly property real graphHeight: 148
    property int hoverIndex: -1
    property string nearLine: ""
    implicitHeight: topInset + graphHeight + 26

    function xAt(i) {
        return periods.length <= 1 ? plotWidth / 2 : i * plotWidth / (periods.length - 1)
    }
    function yAt(line, axis, value) {
        const low = trends.normalized ? line.low : axis.low
        const high = trends.normalized ? line.high : axis.high
        return graphHeight * (high === low ? 0.5 : (high - value) / (high - low))
    }
    function axisInk(axis) {
        const line = axis.lines.find(l => trends.highlighted(l)) || axis.lines[0]
        return line ? trends.ink(line) : trends.win.muted
    }
    function axisOpacity(axis) {
        return !axis.lines.length || axis.lines.some(l => trends.highlighted(l)) ? 1 : 0.35
    }
    // Test both dots and the intervening stroke. A missing value breaks the path.
    function nearestLine(x, y) {
        const step = periods.length > 1 ? plotWidth / (periods.length - 1) : plotWidth
        const first = Math.max(0, Math.floor((x - 8) / step))
        const last = Math.min(periods.length - 1, Math.ceil((x + 8) / step))
        let best = 8 * 8
        let key = ""
        for (const axis of trends.axes) {
            for (const line of axis.lines) {
                let prev = null
                for (let i = first; i <= last; ++i) {
                    const v = line.series.points[i].value
                    if (v == null) { prev = null; continue }
                    const px = xAt(i), py = yAt(line, axis, v)
                    let dx = x - px, dy = y - py
                    if (prev) {
                        const sx = prev.x - px, sy = prev.y - py
                        const t = Math.max(0, Math.min(1, (dx * sx + dy * sy) / (sx * sx + sy * sy)))
                        dx -= t * sx
                        dy -= t * sy
                    }
                    if (dx * dx + dy * dy <= best) {
                        best = dx * dx + dy * dy
                        key = trends.lineKey(line)
                    }
                    prev = { x: px, y: py }
                }
            }
        }
        return key
    }
    function hideTip() {
        if (hoverIndex >= 0 && trends.tip) trends.tip.visible = false
        trends.leave(hover)
        nearLine = ""
        hoverIndex = -1
    }
    onVisibleChanged: if (!visible) hideTip()
    onPeriodsChanged: hideTip()
    Component.onDestruction: hideTip()

    Row {
        id: headings
        Repeater {
            model: plot.axes
            Column {
                required property var modelData
                width: plot.columnWidth
                spacing: 2
                opacity: plot.axisOpacity(modelData)
                Label {
                    width: parent.width - 8
                    text: parent.modelData.name + (plot.trends.pinned === "axis:" + parent.modelData.measure
                                                  ? qsTr(" · pinned") : "")
                    textFormat: Text.PlainText
                    color: plot.axisInk(parent.modelData)
                    font.pixelSize: 12
                    font.weight: Font.DemiBold
                    wrapMode: Text.Wrap
                    maximumLineCount: 3
                    elide: Text.ElideRight
                    horizontalAlignment: Text.AlignRight
                }
                Label {
                    width: parent.width - 8
                    text: parent.modelData.unit
                    visible: text.length > 0 && !parent.modelData.name.toLowerCase().endsWith(text.toLowerCase())
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: plot.axisInk(parent.modelData)
                    font.family: plot.trends.win.mono
                    font.pixelSize: 10
                    horizontalAlignment: Text.AlignRight
                }
            }
        }
    }
    Repeater {
        model: plot.axes
        Item {
            id: column
            required property var modelData
            required property int index
            objectName: "trendsAxis_" + modelData.measure
            readonly property color ink: plot.axisInk(modelData)
            x: index * plot.columnWidth
            width: plot.columnWidth
            height: plot.topInset + plot.graphHeight + 6
            opacity: plot.axisOpacity(modelData)
            Rectangle {
                x: column.width - 2
                y: plot.topInset
                width: 1
                height: plot.graphHeight
                color: column.ink
                opacity: 0.45
            }
            Repeater {
                model: 3
                Item {
                    required property int index
                    y: plot.topInset + index * plot.graphHeight / 2
                    width: column.width
                    Rectangle {
                        x: parent.width - 6
                        width: 5
                        height: 1
                        color: column.ink
                    }
                    Label {
                        width: parent.width - 11
                        y: -height / 2
                        text: plot.trends.win.numberText(column.modelData.high
                              - index * (column.modelData.high - column.modelData.low) / 2)
                              + (plot.trends.normalized ? "%" : "")
                        horizontalAlignment: Text.AlignRight
                        textFormat: Text.PlainText
                        font.family: plot.trends.win.mono
                        font.pixelSize: 11
                        color: column.ink
                    }
                }
            }
            MouseArea {
                id: axisPointer
                anchors.fill: parent
                enabled: column.modelData.lines.length > 0
                readonly property string key: enabled ? plot.trends.axisKey(column.modelData) : ""
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onEntered: plot.trends.pointAt(axisPointer, key)
                onExited: plot.trends.leave(axisPointer)
                onClicked: plot.trends.pin(key)
            }
        }
    }
    Canvas {
        id: canvas
        x: plot.leftGutter - 4
        y: plot.topInset - 4
        width: plot.plotWidth + 8
        height: plot.graphHeight + 8
        readonly property var axes: plot.trends.axes
        readonly property string highlight: plot.trends.highlight
        readonly property bool normalized: plot.trends.normalized
        readonly property var slots: plot.trends.slots
        onAxesChanged: requestPaint()
        onHighlightChanged: requestPaint()
        onNormalizedChanged: requestPaint()
        onSlotsChanged: requestPaint()
        function drawLine(ctx, axis, line, radius) {
            const points = line.series.points
            ctx.strokeStyle = plot.trends.ink(line)
            ctx.fillStyle = ctx.strokeStyle
            ctx.globalAlpha = plot.trends.highlighted(line) ? 1 : 0.35
            ctx.lineWidth = highlight && plot.trends.highlighted(line) ? 2.5 : 1.8
            ctx.beginPath()
            let connected = false
            for (let i = 0; i < points.length; ++i) {
                const v = points[i].value
                if (v == null) { connected = false; continue }
                const x = plot.xAt(i), y = plot.yAt(line, axis, v)
                if (connected) ctx.lineTo(x, y)
                else ctx.moveTo(x, y)
                connected = true
            }
            ctx.stroke()
            ctx.beginPath()
            for (let i = 0; i < points.length; ++i) {
                const v = points[i].value
                if (v == null) continue
                const x = plot.xAt(i), y = plot.yAt(line, axis, v)
                ctx.moveTo(x + radius, y)
                ctx.arc(x, y, radius, 0, 2 * Math.PI)
            }
            ctx.fill()
        }
        onPaint: {
            const ctx = getContext("2d")
            ctx.reset()
            ctx.translate(4, 4)
            ctx.strokeStyle = String(plot.trends.win.border)
            for (let g = 0; g <= 2; ++g) {
                const y = plot.graphHeight * g / 2
                ctx.beginPath()
                ctx.moveTo(0, y)
                ctx.lineTo(plot.plotWidth, y)
                ctx.stroke()
            }
            const radius = Math.max(1.2, Math.min(3, plot.plotWidth / Math.max(1, plot.periods.length) / 3))
            // Paint focused strokes last so they remain legible at intersections.
            for (const foreground of [false, true])
                for (const axis of axes)
                    for (const line of axis.lines)
                        if (plot.trends.highlighted(line) === foreground)
                            drawLine(ctx, axis, line, radius)
        }
    }
    Rectangle {
        visible: plot.hoverIndex >= 0
        x: plot.leftGutter + plot.xAt(plot.hoverIndex)
        y: plot.topInset
        width: 1
        height: plot.graphHeight
        color: plot.trends.win.muted
        opacity: 0.6
    }
    Repeater {
        model: plot.labelCount
        Label {
            required property int index
            readonly property int period: Math.round(index * (plot.periods.length - 1) / Math.max(1, plot.labelCount - 1))
            x: Math.min(plot.width - width, Math.max(0, plot.leftGutter + plot.xAt(period) - width / 2))
            anchors.bottom: parent.bottom
            text: Qt.formatDate(new Date(plot.periods[period] + "T12:00:00"), "d MMM")
            textFormat: Text.PlainText
            color: plot.trends.win.muted
            font.family: plot.trends.win.mono
            font.pixelSize: 11
        }
    }
    MouseArea {
        id: hover
        objectName: "trendsHover"
        x: plot.leftGutter - 4
        y: plot.topInset - 4
        width: plot.plotWidth + 8
        height: plot.graphHeight + 8
        hoverEnabled: true
        cursorShape: plot.nearLine ? Qt.PointingHandCursor : Qt.ArrowCursor
        function showTip(x, y) {
            if (!plot.periods.length || !plot.trends.tip) return
            const key = plot.nearestLine(x - 4, y - 4)
            plot.nearLine = key
            plot.trends.pointAt(hover, key)
            plot.hoverIndex = Math.max(0, Math.min(plot.periods.length - 1,
                Math.round((x - 4) / plot.plotWidth * (plot.periods.length - 1))))
            const tip = plot.trends.tip, win = plot.trends.win
            tip.text = plot.trends.tooltip(plot.hoverIndex)
            const p = mapToItem(win.contentItem, x, y)
            tip.x = Math.max(8, Math.min(win.width - tip.width - 8, p.x + 14))
            tip.y = p.y >= tip.height + 18 ? p.y - tip.height - 10 : Math.min(win.height - tip.height - 8, p.y + 14)
            tip.visible = true
        }
        onPositionChanged: mouse => showTip(mouse.x, mouse.y)
        onEntered: showTip(mouseX, mouseY)
        onExited: plot.hideTip()
        onClicked: mouse => {
            const key = plot.nearestLine(mouse.x - 4, mouse.y - 4)
            if (key) plot.trends.pin(key)
        }
    }
}
