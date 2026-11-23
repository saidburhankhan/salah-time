import QtQuick
import QtQuick.Controls

Item {
    id: plot
    required property var trends
    readonly property var periods: trends.periods
    readonly property real leftGutter: 46
    readonly property real plotWidth: Math.max(1, width - leftGutter - 24)
    readonly property int panelHeight: trends.panels.length > 1 ? 120 : 168
    readonly property int topInset: 28
    readonly property int bottomInset: 9
    readonly property int labelCount: Math.min(periods.length, Math.max(2, Math.min(7, Math.floor(plotWidth / 95))))
    property int hoverIndex: -1
    implicitHeight: trends.panels.length * panelHeight + 22

    function xAt(i) {
        return periods.length <= 1 ? plotWidth / 2 : i * plotWidth / (periods.length - 1)
    }
    function hideTip() {
        if (hoverIndex >= 0 && trends.tip)
            trends.tip.visible = false
        hoverIndex = -1
    }
    onVisibleChanged: if (!visible) hideTip()
    onPeriodsChanged: hideTip()
    Component.onDestruction: hideTip()

    Repeater {
        model: plot.trends.panels
        Item {
            id: panel
            required property var modelData
            required property int index
            y: index * plot.panelHeight
            width: plot.width
            height: plot.panelHeight
            readonly property bool relative: plot.trends.normalized
            readonly property var lines: modelData.lines
            readonly property real low: relative ? 0 : Math.min(...lines.map(l => l.series.scale_max ? 1 : Math.min(0, l.low)))
            readonly property real high: relative ? 100 : Math.max(low + 1, ...lines.map(l => Math.max(l.series.scale_max || 0, l.high)))
            function ordinate(line, v) {
                if (relative)
                    return line.high === line.low ? 0.5 : (v - line.low) / (line.high - line.low)
                return (v - low) / (high - low)
            }
            Label {
                x: plot.leftGutter
                text: panel.modelData.title
                textFormat: Text.PlainText
                color: plot.trends.win.muted
                font.pixelSize: 12
            }
            Repeater {
                model: 3
                Label {
                    required property int index
                    width: plot.leftGutter - 8
                    y: plot.topInset + index * (panel.height - plot.topInset - plot.bottomInset) / 2 - height / 2
                    horizontalAlignment: Text.AlignRight
                    text: plot.trends.win.numberText(panel.high - index * (panel.high - panel.low) / 2) + (panel.relative ? "%" : "")
                    textFormat: Text.PlainText
                    color: plot.trends.win.muted
                    font.family: plot.trends.win.mono
                    font.pixelSize: 11
                    fontSizeMode: Text.HorizontalFit
                    minimumPixelSize: 8
                }
            }
            Canvas {
                id: canvas
                anchors.fill: parent
                // Canvas depends on more than its size: a legend click, a new
                // response or a switch of units can leave the bounds unchanged.
                readonly property var series: panel.lines
                readonly property var slots: plot.trends.slots
                readonly property bool normalized: panel.relative
                onSeriesChanged: requestPaint()
                onSlotsChanged: requestPaint()
                onNormalizedChanged: requestPaint()
                onPaint: {
                    let ctx = getContext("2d")
                    ctx.reset()
                    let h = height - plot.topInset - plot.bottomInset
                    ctx.translate(plot.leftGutter, plot.topInset)
                    ctx.strokeStyle = String(plot.trends.win.border)
                    ctx.lineWidth = 1
                    for (let g = 0; g <= 2; ++g) {
                        let y = h * g / 2
                        ctx.beginPath()
                        ctx.moveTo(0, y)
                        ctx.lineTo(plot.plotWidth, y)
                        ctx.stroke()
                    }
                    let radius = Math.max(1.2, Math.min(3, plot.plotWidth / Math.max(1, plot.periods.length) / 3))
                    for (let line of series) {
                        let points = line.series.points
                        let color = plot.trends.ink(line)
                        ctx.strokeStyle = color
                        ctx.fillStyle = color
                        ctx.lineWidth = 2
                        ctx.beginPath()
                        let connected = false
                        for (let i = 0; i < points.length; ++i) {
                            let v = points[i].value
                            if (v === null || v === undefined) {
                                connected = false
                                continue
                            }
                            let x = plot.xAt(i), y = h * (1 - panel.ordinate(line, v))
                            if (connected) ctx.lineTo(x, y)
                            else ctx.moveTo(x, y)
                            connected = true
                        }
                        ctx.stroke()
                        ctx.beginPath()
                        for (let i = 0; i < points.length; ++i) {
                            let v = points[i].value
                            if (v === null || v === undefined) continue
                            let x = plot.xAt(i), y = h * (1 - panel.ordinate(line, v))
                            ctx.moveTo(x + radius, y)
                            ctx.arc(x, y, radius, 0, 2 * Math.PI)
                        }
                        ctx.fill()
                    }
                }
            }
            Rectangle {
                visible: plot.hoverIndex >= 0
                x: plot.leftGutter + plot.xAt(plot.hoverIndex)
                y: plot.topInset
                width: 1
                height: parent.height - y - plot.bottomInset
                color: plot.trends.win.muted
            }
        }
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
        x: plot.leftGutter
        width: plot.plotWidth
        height: plot.height - 22
        acceptedButtons: Qt.NoButton
        hoverEnabled: true
        function showTip(x, y) {
            if (!plot.periods.length || !plot.trends.tip) return
            plot.hoverIndex = Math.max(0, Math.min(plot.periods.length - 1, Math.round(x / width * (plot.periods.length - 1))))
            let tip = plot.trends.tip
            let win = plot.trends.win
            tip.text = plot.trends.tooltip(plot.hoverIndex)
            let p = mapToItem(win.contentItem, x, y)
            tip.x = Math.max(8, Math.min(win.width - tip.width - 8, p.x + 14))
            tip.y = p.y >= tip.height + 18 ? p.y - tip.height - 10 : Math.min(win.height - tip.height - 8, p.y + 14)
            tip.visible = true
        }
        onPositionChanged: mouse => showTip(mouse.x, mouse.y)
        onEntered: showTip(mouseX, mouseY)
        onExited: plot.hideTip()
    }
}
