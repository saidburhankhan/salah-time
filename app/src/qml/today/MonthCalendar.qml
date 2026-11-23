import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

Card {
    id: monthCalendar
    property var win
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 6
        RowLayout {
            Layout.fillWidth: true
            ToolButton {
                text: "‹"
                onClicked: backend.selectDay(monthCalendar.win.iso(monthCalendar.win.monthStep(backend.selectedDay, -1)))
            }
            Label {
                Layout.fillWidth: true
                text: Qt.locale().toString(
                    Date.fromLocaleString(Qt.locale(), backend.selectedDay, "yyyy-MM-dd"),
                    "MMMM yyyy")
                horizontalAlignment: Text.AlignHCenter
                color: monthCalendar.win.text
                font.pixelSize: 16
                font.weight: Font.Medium
            }
            ToolButton {
                text: qsTr("Today")
                font.pixelSize: 11
                onClicked: backend.jumpToday()
            }
            ToolButton {
                text: "›"
                onClicked: backend.selectDay(monthCalendar.win.iso(monthCalendar.win.monthStep(backend.selectedDay, 1)))
            }
        }
        GridLayout {
            columns: 7
            Layout.fillWidth: true
            rowSpacing: 5
            columnSpacing: 4
            Repeater {
                model: ["M", "T", "W", "T", "F", "S", "S"]
                Label {
                    required property string modelData
                    Layout.fillWidth: true
                    text: modelData
                    color: monthCalendar.win.muted
                    font.family: monthCalendar.win.mono
                    font.pixelSize: 11
                    horizontalAlignment: Text.AlignHCenter
                }
            }
            Repeater {
                model: 42
                Rectangle {
                    required property int index
                    property var date: monthCalendar.win.calendarDate(index)
                    property string dayIso: monthCalendar.win.iso(date)
                    Layout.fillWidth: true
                    height: 34
                    radius: 17
                    color: dayIso === backend.selectedDay ? monthCalendar.win.selectedFill : "transparent"
                    border.width: dayIso === backend.selectedDay || dayIso === backend.todayDay ? 1 : 0
                    border.color: dayIso === backend.selectedDay ? monthCalendar.win.selectedBorder : "#5f5a51"
                    Label {
                        anchors.centerIn: parent
                        text: parent.date.getDate()
                        color: parent.dayIso === backend.selectedDay
                            ? monthCalendar.win.selectedText
                            : parent.date.getMonth() === monthCalendar.win.calendarDate(15).getMonth()
                                ? "#d5d1c9"
                                : "#5f5a51"
                        font.family: monthCalendar.win.mono
                        font.pixelSize: 13
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: backend.selectDay(parent.dayIso)
                    }
                }
            }
        }
    }
}
