import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: chart
    property var win
    property var tip
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
                model: backend.statsDaily
                Item {
                    id: prayerDay
                    required property string day
                    required property int prayers
                    required property string tooltip
                    required property int index
                    width: (prayerPlot.width - Math.max(0, backend.statsDaily.count - 1) * 2)
                        / Math.max(1, backend.statsDaily.count)
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
                                color: index < prayerDay.prayers ? theme.accent : "#45423b"
                            }
                        }
                    }
                    Label {
                        anchors.top: parent.bottom
                        anchors.topMargin: 7
                        anchors.horizontalCenter: parent.horizontalCenter
                        color: chart.win.muted
                        font.family: chart.win.mono
                        font.pixelSize: 10
                        text: {
                            var d = Date.fromLocaleString(Qt.locale(), prayerDay.day, "yyyy-MM-dd")
                            if (chart.win.statsMode === "week")
                                return Qt.locale("en_US").toString(d, "ddd").charAt(0)
                            return (prayerDay.index % Math.max(1, Math.ceil(backend.statsDaily.count / 12)) === 0
                                || prayerDay.index === backend.statsDaily.count - 1) ? d.getDate() : ""
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        hoverEnabled: true
                        onEntered: {
                            chart.tip.text = prayerDay.tooltip
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
