import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: chart
    objectName: "dayChart"
    property var chartData
    property var win
    property var tip
    property var rows: []
    property var axis: ({ lines: 3, step: 1, maximum: 3 })
    property real maximum: 3
    function refreshRows() {
        rows = win ? win.selectedDaily() : []
        axis = win ? win.niceAxis(chartData ? chartData[1] : "done")
            : ({ lines: 3, step: 1, maximum: 3 })
        maximum = axis.maximum
    }
    Component.onCompleted: refreshRows()
    onWinChanged: refreshRows()
    onChartDataChanged: refreshRows()
    Connections {
        target: backend
        function onChanged() { chart.refreshRows() }
    }
    Connections {
        target: chart.win
        function onSelectedStatsItemChanged() { chart.refreshRows() }
    }
    width: parent.width
    height: 218
    radius: theme.radius
    color: win ? win.panel : "#1b1a17"
    border.color: win ? win.border : "#34312a"
    Label {
        x: 18
        y: 15
        text: chart.chartData[0]
        color: chart.win.text
        font.pixelSize: 15
        font.weight: Font.Medium
    }
    Item {
        id: plot
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.leftMargin: 52
        anchors.rightMargin: 18
        anchors.topMargin: 48
        anchors.bottomMargin: 34
        Repeater {
            model: chart.axis.lines + 1
            Item {
                required property int index
                width: plot.width
                height: 1
                y: index * (plot.height - 1) / chart.axis.lines
                Rectangle {
                    width: parent.width
                    height: 1
                    color: "#302e29"
                }
                Label {
                    x: -45
                    y: -7
                    width: 38
                    horizontalAlignment: Text.AlignRight
                    color: chart.win.muted
                    font.family: chart.win.mono
                    font.pixelSize: 10
                    text: chart.win.axisLabel(chart.chartData[1],
                        (chart.axis.lines - index) * chart.axis.step)
                }
            }
        }
        Row {
            anchors.fill: parent
            spacing: 2
            Repeater {
                id: rowRepeater
                model: chart.rows
                Item {
                    id: taskChartDay
                    required property var modelData
                    required property int index
                    objectName: "taskChartDay" + index
                    width: (plot.width - Math.max(0, chart.rows.length - 1) * 2) / Math.max(1, chart.rows.length)
                    height: plot.height
                    property var dayRow: modelData
                    property bool future: dayRow.day > backend.todayDay
                    Rectangle {
                        visible: chart.chartData[1] === "done"
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Math.max(3, Math.min(42, parent.width - 3))
                        height: plot.height * (parent.dayRow.planned || 0) / chart.maximum
                        radius: 4
                        color: "#45423b"
                        opacity: future ? .3 : 1
                    }
                    Rectangle {
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Math.max(3, Math.min(42, parent.width - 3))
                        height: plot.height * Number(parent.dayRow[chart.chartData[1]] || 0) / chart.maximum
                        radius: 4
                        color: theme.accent
                        opacity: future ? .22 : .9
                    }
                    Rectangle {
                        visible: parent.dayRow.day === backend.todayDay
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Math.max(5, Math.min(46, parent.width - 1))
                        height: parent.height
                        color: "transparent"
                        border.color: chart.win.text
                        border.width: 1
                        opacity: .55
                    }
                    Label {
                        anchors.top: parent.bottom
                        anchors.topMargin: 7
                        anchors.horizontalCenter: parent.horizontalCenter
                        color: future ? "#5f5b54" : chart.win.muted
                        font.family: chart.win.mono
                        font.pixelSize: 10
                        text: {
                            var d = Date.fromLocaleString(Qt.locale(), modelData.day, "yyyy-MM-dd")
                            if (chart.win.statsMode === "week")
                                return Qt.locale("en_US").toString(d, "ddd").charAt(0)
                            return (index % Math.max(1, Math.ceil(chart.rows.length / 12)) === 0
                                || index === chart.rows.length - 1) ? d.getDate() : ""
                        }
                    }
                    MouseArea {
                        objectName: "taskChartHit"
                        anchors.fill: parent
                        anchors.leftMargin: -2
                        anchors.rightMargin: -2
                        hoverEnabled: true
                        onEntered: {
                            chart.tip.text = chart.win.tooltipTextAt(taskChartDay.index)
                            chart.tip.visible = true
                            var p = mapToItem(chart.win.contentItem, width / 2, 0)
                            chart.tip.x = Math.max(230, Math.min(chart.win.width - chart.tip.width - 10,
                                p.x - chart.tip.width / 2))
                            chart.tip.y = Math.max(10, p.y - chart.tip.height - 8)
                        }
                        onExited: chart.tip.visible = false
                    }
                }
            }
        }
    }
}
