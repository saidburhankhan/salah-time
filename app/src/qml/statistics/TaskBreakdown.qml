import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: taskStatsRow
    required property string itemId
    required property string title
    required property string block
    required property string scheduleText
    required property string measure
    required property int plannedDays
    required property int doneDays
    required property int minutes
    required property int count
    required property var series
    property var win
    width: parent.width
    height: 88
    radius: theme.radius
    color: win.selectedStatsItem === itemId ? "#292722" : win.panel
    border.color: win.selectedStatsItem === itemId ? theme.accent : win.border
    RowLayout {
        anchors.fill: parent
        anchors.margins: 15
        spacing: 16
        ColumnLayout {
            Layout.preferredWidth: 260
            spacing: 4
            Label {
                text: taskStatsRow.title
                color: taskStatsRow.win.text
                font.pixelSize: 14
                font.weight: Font.Medium
                Layout.fillWidth: true
                elide: Text.ElideRight
            }
            Label {
                text: taskStatsRow.win.joinedDetail(taskStatsRow.block, taskStatsRow.scheduleText)
                color: taskStatsRow.win.muted
                font.family: taskStatsRow.win.mono
                font.pixelSize: 11
            }
        }
        ColumnLayout {
            Layout.preferredWidth: 190
            spacing: 6
            Label {
                text: (taskStatsRow.doneDays || 0) + " / "
                    + (taskStatsRow.plannedDays || 0) + qsTr(" days")
                color: taskStatsRow.win.muted
                font.family: taskStatsRow.win.mono
                font.pixelSize: 11
            }
            Rectangle {
                Layout.fillWidth: true
                height: 4
                radius: 2
                color: "#403d37"
                Rectangle {
                    height: parent.height
                    width: parent.width * (taskStatsRow.plannedDays
                        ? taskStatsRow.doneDays / taskStatsRow.plannedDays : 0)
                    radius: 2
                    color: theme.accent
                }
            }
        }
        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: 42
            Row {
                anchors.fill: parent
                spacing: 2
                Repeater {
                    model: taskStatsRow.series
                    Rectangle {
                        required property int minutes
                        width: (parent.parent.width
                            - Math.max(0, taskStatsRow.series.count - 1) * 2)
                            / Math.max(1, taskStatsRow.series.count)
                        height: Math.max(2, parent.parent.height * Math.min(1, minutes / 60))
                        anchors.bottom: parent.bottom
                        radius: 2
                        color: "#777168"
                    }
                }
            }
        }
        Label {
            Layout.preferredWidth: 90
            text: taskStatsRow.win.formatMinutes(taskStatsRow.minutes)
            color: taskStatsRow.win.text
            font.family: taskStatsRow.win.mono
            font.pixelSize: 14
            horizontalAlignment: Text.AlignRight
        }
        Label {
            Layout.preferredWidth: 76
            text: taskStatsRow.measure === "count"
                ? (taskStatsRow.count || 0) + " ×" : "—"
            color: taskStatsRow.win.muted
            font.family: taskStatsRow.win.mono
            font.pixelSize: 12
            horizontalAlignment: Text.AlignRight
        }
    }
    MouseArea {
        anchors.fill: parent
        onClicked: taskStatsRow.win.selectedStatsItem = taskStatsRow.itemId
    }
}
