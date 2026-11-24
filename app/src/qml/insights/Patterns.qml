import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../statistics"

ColumnLayout {
    id: patterns
    required property var win
    required property var report
    required property var blockLabel
    property string group: "block"
    readonly property var rows: report["by_" + group] || []
    readonly property int maxMinutes: rows.reduce((largest, row) => Math.max(largest, row.minutes), 1)
    spacing: 14

    function groupLabel(row) {
        if (group === "block") return blockLabel(row.block)
        if (group === "hour") return String(row.hour).padStart(2, "0") + ":00"
        return Qt.locale().dayName(row.weekday % 7, Locale.LongFormat)
    }

    component Cell: Label {
        textFormat: Text.PlainText
        font.pixelSize: 13
        color: patterns.win.text
        horizontalAlignment: Text.AlignRight
        font.features: { "tnum": 1 }
    }
    component MeasureRow: RowLayout {
        id: measure
        property var entry: null
        readonly property bool header: !entry
        objectName: header ? "insightGroupHeader" : "insightGroupRow_" + entry[patterns.group]
        Layout.fillWidth: true
        spacing: 12
        Cell {
            Layout.preferredWidth: 100
            horizontalAlignment: Text.AlignLeft
            text: measure.header ? (patterns.group === "hour" ? qsTr("Hour") : patterns.group === "weekday" ? qsTr("Weekday") : qsTr("Block"))
                                 : patterns.groupLabel(measure.entry)
            color: measure.header ? patterns.win.muted : patterns.win.text
            elide: Text.ElideRight
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 6
            radius: 3
            color: measure.header ? "transparent" : patterns.win.border
            Rectangle {
                width: measure.header ? 0 : parent.width * measure.entry.minutes / patterns.maxMinutes
                height: parent.height
                radius: 3
                color: patterns.win.selectedBorder
            }
        }
        Cell {
            Layout.preferredWidth: 60
            text: measure.header ? qsTr("Minutes") : String(measure.entry.minutes)
            color: measure.header ? patterns.win.muted : patterns.win.text
        }
        Cell {
            Layout.preferredWidth: 70
            text: measure.header ? qsTr("Sessions") : String(measure.entry.sessions)
            color: patterns.win.muted
        }
        Cell {
            Layout.preferredWidth: 72
            text: measure.header ? qsTr("Avg focus") : patterns.win.numberText(measure.entry.focus_avg)
            color: measure.header ? patterns.win.muted : patterns.win.text
        }
        Cell {
            Layout.preferredWidth: 103
            text: measure.header ? qsTr("Distractions/h") : patterns.win.numberText(measure.entry.distractions_per_hour)
            color: measure.header ? patterns.win.muted : patterns.win.text
        }
    }

    Pane {
        Layout.fillWidth: true
        padding: 18
        background: Card { win: patterns.win }
        contentItem: ColumnLayout {
            spacing: 9
            RowLayout {
                Layout.fillWidth: true
                spacing: 8
                Repeater {
                    model: [{key: "block", title: qsTr("By prayer block")},
                            {key: "hour", title: qsTr("By hour of day")},
                            {key: "weekday", title: qsTr("By weekday")}]
                    ChoiceChip {
                        required property var modelData
                        objectName: "insightGroup_" + modelData.key
                        win: patterns.win
                        text: modelData.title
                        selected: patterns.group === modelData.key
                        onClicked: patterns.group = modelData.key
                    }
                }
                Item { Layout.fillWidth: true }
            }
            Label {
                visible: patterns.group === "hour"
                text: qsTr("Timed sessions, grouped by local start time")
                color: patterns.win.muted
                font.pixelSize: 13
            }
            MeasureRow {}
            Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: patterns.win.border }
            Repeater {
                model: patterns.rows
                MeasureRow {
                    required property var modelData
                    entry: modelData
                }
            }
            Label {
                visible: patterns.rows.length === 0
                text: qsTr("No sessions in this group.")
                color: patterns.win.muted
            }
        }
    }
    LineChart {
        objectName: "insightsWeekly"
        Layout.fillWidth: true
        Layout.preferredHeight: 210
        win: patterns.win
        weekly: true
        series: ({
            name: qsTr("Distractions per hour by week"),
            rollup: "avg",
            points: (patterns.report.distractions_by_week || []).map(w => ({day: w.week, value: w.per_hour}))
        })
    }
}
