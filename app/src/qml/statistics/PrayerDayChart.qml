import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: chart
    property var win
    property var tip
    property var rows: []
    function refreshRows() { rows = win ? win.selectedDaily() : [] }
    Component.onCompleted: refreshRows()
    onWinChanged: refreshRows()
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
        text: qsTr("Prayers per day")
        color: chart.win.text
        font.pixelSize: 15
        font.weight: Font.Medium
    }
    Item {
        id: prayerPlot
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.leftMargin: 52
        anchors.rightMargin: 18
        anchors.topMargin: 48
        anchors.bottomMargin: 34
        Repeater {
            model: 6
            Item {
                required property int index
                width: prayerPlot.width
                height: 1
                y: index * (prayerPlot.height - 1) / 5
                Rectangle {
                    width: parent.width
                    height: 1
                    color: "#302e29"
                }
                Label {
                    x: -45
                    y: -7
                    width: 38
                    text: String(5 - index)
                    horizontalAlignment: Text.AlignRight
                    color: chart.win.muted
                    font.family: chart.win.mono
                    font.pixelSize: 10
                }
            }
        }
        Row {
            anchors.fill: parent
            spacing: 2
            Repeater {
                model: chart.rows
                Item {
                    id: prayerDay
                    required property var modelData
                    required property int index
                    property var dayRow: modelData
                    width: (prayerPlot.width - Math.max(0, chart.rows.length - 1) * 2) / Math.max(1, chart.rows.length)
                    height: prayerPlot.height
                    Column {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.bottom: parent.bottom
                        spacing: Math.max(2, (prayerPlot.height - 35) / 4)
                        Repeater {
                            model: 5
                            Rectangle {
                                required property int index
                                width: 7
                                height: 7
                                radius: 4
                                color: index < (prayerDay.dayRow.prayers || 0) ? theme.accent : "#45423b"
                            }
                        }
                    }
                    Label {
                        anchors.top: parent.bottom
                        anchors.topMargin: 7
                        anchors.horizontalCenter: parent.horizontalCenter
                        color: parent.dayRow.day > backend.todayDay ? "#5f5b54" : chart.win.muted
                        font.family: chart.win.mono
                        font.pixelSize: 10
                        text: {
                            var d = Date.fromLocaleString(Qt.locale(), parent.dayRow.day, "yyyy-MM-dd")
                            if (chart.win.statsMode === "week")
                                return Qt.locale("en_US").toString(d, "ddd").charAt(0)
                            return (parent.index % Math.max(1, Math.ceil(chart.rows.length / 12)) === 0
                                || parent.index === chart.rows.length - 1) ? d.getDate() : ""
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        hoverEnabled: true
                        onEntered: {
                            chart.tip.text = chart.win.tooltipTextAt(parent.index)
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
