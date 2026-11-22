import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../statistics"

ScrollView {
    id: statsScroll
    objectName: "statsScroll"
    property var win
    property var tip
    clip: true
    Column {
        width: statsScroll.availableWidth
        spacing: 16
        RangeBar {
            win: statsScroll.win
        }
        Row {
            width: parent.width
            spacing: 12
            Repeater {
                model: [
                    [qsTr("Tasks done"),
                        (backend.stats.totals || {}).done + " / " + (backend.stats.totals || {}).planned,
                        ((backend.stats.totals || {}).planned
                            ? Math.round(100 * (backend.stats.totals || {}).done / (backend.stats.totals || {}).planned)
                            : 0) + "% completed"],
                    [qsTr("Time spent"),
                        statsScroll.win.formatMinutes((backend.stats.totals || {}).minutes),
                        qsTr("%1 active days").arg((backend.stats.totals || {}).active_days || 0)],
                    [qsTr("Counts"),
                        String((backend.stats.totals || {}).count || 0),
                        qsTr("logged in this range")],
                    [qsTr("Prayers"),
                        (backend.stats.totals || {}).prayers + " / " + (backend.stats.prayer_slots || 0),
                        qsTr("five daily prayers")]
                ]
                SummaryTile {
                    tileData: modelData
                    win: statsScroll.win
                }
            }
        }
        RowLayout {
            width: parent.width
            Label {
                text: statsScroll.win.selectedStatsItem ? qsTr("Filtered task") : qsTr("Daily activity")
                color: statsScroll.win.text
                font.pixelSize: 18
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }
            AppButton {
                win: statsScroll.win
                visible: statsScroll.win.selectedStatsItem
                text: qsTr("All tasks  ×")
                onClicked: statsScroll.win.selectedStatsItem = ""
            }
            Rectangle {
                width: 138
                height: 34
                radius: 7
                color: statsScroll.win.panel
                border.color: statsScroll.win.border
                Row {
                    anchors.fill: parent
                    Repeater {
                        model: [[false, qsTr("Chart")], [true, qsTr("Table")]]
                        Rectangle {
                            required property var modelData
                            width: 69
                            height: 34
                            radius: 6
                            color: statsScroll.win.statsTableVisible === modelData[0] ? "#3a3730" : "transparent"
                            Label {
                                anchors.centerIn: parent
                                text: modelData[1]
                                color: statsScroll.win.statsTableVisible === modelData[0]
                                    ? statsScroll.win.text : statsScroll.win.muted
                                font.pixelSize: 12
                            }
                            MouseArea {
                                anchors.fill: parent
                                onClicked: statsScroll.win.statsTableVisible = modelData[0]
                            }
                        }
                    }
                }
            }
        }
        Column {
            visible: !statsScroll.win.statsTableVisible
            width: parent.width
            spacing: 12
            Repeater {
                model: [[qsTr("Tasks done per day"), "done"], [qsTr("Time spent per day"), "minutes"]]
                DayColumnChart {
                    chartData: modelData
                    win: statsScroll.win
                    tip: statsScroll.tip
                }
            }
            PrayerDayChart {
                win: statsScroll.win
                tip: statsScroll.tip
            }
        }
        DayTable {
            visible: statsScroll.win.statsTableVisible
            win: statsScroll.win
        }
        Label {
            text: qsTr("By task")
            color: statsScroll.win.text
            font.pixelSize: 18
            font.weight: Font.DemiBold
            topPadding: 4
        }
        Repeater {
            model: (backend.stats.items || []).slice().sort(function(a, b) {
                return (b.minutes || 0) - (a.minutes || 0)
            })
            TaskBreakdown {
                taskData: modelData
                win: statsScroll.win
            }
        }
        Label {
            visible: !backend.busy && !(backend.stats.daily || []).some(function(x) {
                return x.done || x.minutes || x.count || x.prayers
            })
            width: parent.width
            height: 80
            text: qsTr("No activity in this range")
            color: statsScroll.win.muted
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
        Label {
            visible: backend.busy
            width: parent.width
            height: 50
            text: qsTr("Loading statistics…")
            color: statsScroll.win.muted
            horizontalAlignment: Text.AlignHCenter
        }
    }
    RangeDialog {
        win: statsScroll.win
    }
}
