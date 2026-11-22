import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: taskStatsRow
    property var taskData
    property var win
    property var rowData: taskData
    width: parent.width
    height: 88
    radius: theme.radius
    color: win.selectedStatsItem === taskData.id ? "#292722" : win.panel
    border.color: win.selectedStatsItem === taskData.id ? theme.accent : win.border
    RowLayout {
        anchors.fill: parent
        anchors.margins: 15
        spacing: 16
        ColumnLayout {
            Layout.preferredWidth: 260
            spacing: 4
            Label {
                text: taskStatsRow.taskData.title
                color: taskStatsRow.win.text
                font.pixelSize: 14
                font.weight: Font.Medium
                Layout.fillWidth: true
                elide: Text.ElideRight
            }
            Label {
                text: taskStatsRow.win.joinedDetail(taskStatsRow.taskData.block, taskStatsRow.taskData.schedule_text || "")
                color: taskStatsRow.win.muted
                font.family: taskStatsRow.win.mono
                font.pixelSize: 11
            }
        }
        ColumnLayout {
            Layout.preferredWidth: 190
            spacing: 6
            Label {
                text: (taskStatsRow.taskData.done_days || 0) + " / "
                    + (taskStatsRow.taskData.planned_days || 0) + qsTr(" days")
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
                    width: parent.width * (taskStatsRow.taskData.planned_days
                        ? taskStatsRow.taskData.done_days / taskStatsRow.taskData.planned_days : 0)
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
                    model: taskStatsRow.rowData.series || []
                    Rectangle {
                        required property var modelData
                        width: (parent.parent.width
                            - Math.max(0, (taskStatsRow.rowData.series || []).length - 1) * 2)
                            / Math.max(1, (taskStatsRow.rowData.series || []).length)
                        height: Math.max(2, parent.parent.height * Math.min(1, (modelData.minutes || 0) / 60))
                        anchors.bottom: parent.bottom
                        radius: 2
                        color: "#777168"
                    }
                }
            }
        }
        Label {
            Layout.preferredWidth: 90
            text: taskStatsRow.win.formatMinutes(taskStatsRow.taskData.minutes)
            color: taskStatsRow.win.text
            font.family: taskStatsRow.win.mono
            font.pixelSize: 14
            horizontalAlignment: Text.AlignRight
        }
        Label {
            Layout.preferredWidth: 76
            text: taskStatsRow.taskData.measure === "count"
                ? (taskStatsRow.taskData.count || 0) + " ×" : "—"
            color: taskStatsRow.win.muted
            font.family: taskStatsRow.win.mono
            font.pixelSize: 12
            horizontalAlignment: Text.AlignRight
        }
    }
    MouseArea {
        anchors.fill: parent
        onClicked: taskStatsRow.win.selectedStatsItem = taskStatsRow.taskData.id
    }
}
