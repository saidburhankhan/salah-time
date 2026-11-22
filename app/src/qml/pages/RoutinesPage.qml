import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ScrollView {
    id: routinesScroll
    property var win
    clip: true
    Column {
        width: routinesScroll.availableWidth
        spacing: 12
        Label {
            text: qsTr("Your repeating rhythm")
            color: routinesScroll.win.muted
            font.pixelSize: 14
            bottomPadding: 8
        }
        Repeater {
            model: backend.routines
            Rectangle {
                id: routineRow
                required property string itemId
                required property string title
                required property string block
                required property string scheduleText
                required property int plannedDays
                required property int doneDays
                required property string nextOccurrence
                required property int minutes
                required property int count
                required property var series
                width: parent.width
                height: 116
                radius: theme.radius
                color: routinesScroll.win.panel
                border.color: routinesScroll.win.border
                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 18
                    spacing: 24
                    ColumnLayout {
                        Layout.preferredWidth: 270
                        spacing: 5
                        Label {
                            text: routineRow.title
                            color: routinesScroll.win.text
                            font.pixelSize: 17
                            font.weight: Font.Medium
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }
                        Label {
                            text: routinesScroll.win.joinedDetail(routineRow.block, routineRow.scheduleText)
                            color: routinesScroll.win.muted
                            font.family: routinesScroll.win.mono
                            font.pixelSize: 12
                        }
                        Label {
                            text: routineRow.plannedDays === 0 && routineRow.nextOccurrence
                                ? qsTr("Starts %1").arg(routinesScroll.win.shortDate(routineRow.nextOccurrence))
                                : qsTr("%1 of %2 days").arg(routineRow.doneDays).arg(routineRow.plannedDays)
                            color: "#c7c2b9"
                            font.pixelSize: 13
                        }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        spacing: 5
                        Repeater {
                            model: routineRow.series
                            Item {
                                required property bool done
                                required property int minutes
                                required property int count
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                Rectangle {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    anchors.bottom: parent.bottom
                                    width: Math.max(5, parent.width - 3)
                                    height: done
                                        ? Math.max(7, Math.min(parent.height - 12, (minutes || count || 1) * 2))
                                        : 5
                                    radius: 3
                                    color: done ? theme.accent : "#38352f"
                                }
                            }
                        }
                    }
                    ColumnLayout {
                        Layout.preferredWidth: 120
                        Label {
                            text: qsTr("%1 min").arg(routineRow.minutes || 0)
                            color: routinesScroll.win.text
                            font.family: routinesScroll.win.mono
                            font.pixelSize: 16
                            Layout.alignment: Qt.AlignRight
                        }
                        Label {
                            visible: routineRow.count > 0
                            text: qsTr("%1 count").arg(routineRow.count || 0)
                            color: routinesScroll.win.text
                            font.family: routinesScroll.win.mono
                            font.pixelSize: 14
                            Layout.alignment: Qt.AlignRight
                        }
                        Label {
                            text: qsTr("total")
                            color: routinesScroll.win.muted
                            Layout.alignment: Qt.AlignRight
                        }
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    onClicked: routinesScroll.win.openEntry(routineRow.itemId)
                }
            }
        }
    }
}
