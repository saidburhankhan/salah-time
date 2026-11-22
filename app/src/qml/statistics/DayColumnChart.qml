import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: chart
    objectName: "dayChart"
    property var chartData
    property var win
    property var tip
    property bool timeChart: chartData ? chartData[1] === "minutes" : false
    property int axisLines: timeChart && backend.statsTicks.count > 0
        ? Math.max(1, backend.statsTicks.count - 1) : 3
    property real maximum: timeChart && backend.statsTicks.count > 0
        ? Math.max(1, backend.statsTicks.lastMinutes)
        : Math.max(1, Math.ceil(Math.max(backend.statsDaily.maxPlanned, backend.statsDaily.maxDone)))
    function axisLabel(index) {
        if (timeChart && backend.statsTicks.count > 0)
            return backend.statsTicks.labelAt(backend.statsTicks.count - 1 - index)
        return String(Math.round((axisLines - index) * maximum / axisLines))
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
            model: chart.axisLines + 1
            Item {
                required property int index
                width: plot.width
                height: 1
                y: index * (plot.height - 1) / chart.axisLines
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
                    text: chart.axisLabel(index)
                }
            }
        }
        Row {
            anchors.fill: parent
            spacing: 2
            Repeater {
                id: rowRepeater
                model: backend.statsDaily
                Item {
                    id: taskChartDay
                    required property string day
                    required property bool planned
                    required property bool done
                    required property int minutes
                    required property int count
                    required property string tooltip
                    required property int index
                    objectName: "taskChartDay" + index
                    width: (plot.width - Math.max(0, backend.statsDaily.count - 1) * 2)
                        / Math.max(1, backend.statsDaily.count)
                    height: plot.height
                    Rectangle {
                        visible: !chart.timeChart
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Math.max(3, Math.min(42, parent.width - 3))
                        height: plot.height * (taskChartDay.planned ? 1 : 0) / chart.maximum
                        radius: 4
                        color: "#45423b"
                    }
                    Rectangle {
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Math.max(3, Math.min(42, parent.width - 3))
                        height: plot.height * Number(chart.timeChart ? taskChartDay.minutes : (taskChartDay.done ? 1 : 0)) / chart.maximum
                        radius: 4
                        color: theme.accent
                        opacity: .9
                    }
                    Rectangle {
                        visible: taskChartDay.day === backend.todayDay
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
                        color: chart.win.muted
                        font.family: chart.win.mono
                        font.pixelSize: 10
                        text: {
                            var d = Date.fromLocaleString(Qt.locale(), taskChartDay.day, "yyyy-MM-dd")
                            if (chart.win.statsMode === "week")
                                return Qt.locale("en_US").toString(d, "ddd").charAt(0)
                            return (taskChartDay.index % Math.max(1, Math.ceil(backend.statsDaily.count / 12)) === 0
                                || taskChartDay.index === backend.statsDaily.count - 1) ? d.getDate() : ""
                        }
                    }
                    MouseArea {
                        objectName: "taskChartHit"
                        anchors.fill: parent
                        anchors.leftMargin: -2
                        anchors.rightMargin: -2
                        hoverEnabled: true
                        onEntered: {
                            chart.tip.text = taskChartDay.tooltip
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
