import QtQuick
import QtQuick.Controls

Rectangle {
    id: tableCard
    property var win
    width: parent.width
    height: 74 + backend.statsDailyNewestCount * 35
    radius: theme.radius
    color: win ? win.panel : "#1b1a17"
    border.color: win ? win.border : "#34312a"
    Column {
        anchors.fill: parent
        anchors.margins: 14
        Row {
            width: parent.width
            height: 34
            Repeater {
                model: [[qsTr("Date"), .34], [qsTr("Done / Planned"), .2], [qsTr("Time"), .16],
                    [qsTr("Counts"), .15], [qsTr("Prayers"), .15]]
                Label {
                    required property var modelData
                    required property int index
                    width: parent.width * modelData[1]
                    text: modelData[0]
                    color: tableCard.win.muted
                    font.pixelSize: 11
                    horizontalAlignment: index ? Text.AlignRight : Text.AlignLeft
                }
            }
        }
        Repeater {
            model: backend.statsDailyNewest
            Row {
                required property string day
                required property bool planned
                required property bool done
                required property int minutes
                required property int count
                required property int prayers
                width: tableCard.width - 28
                height: 35
                opacity: (done || minutes || count || prayers) ? .95 : .38
                Label {
                    width: parent.width * .34
                    text: Qt.locale("en_US").toString(
                        Date.fromLocaleString(Qt.locale(), parent.day, "yyyy-MM-dd"), "ddd, d MMM")
                    color: tableCard.win.text
                    font.pixelSize: 12
                }
                Label {
                    width: parent.width * .2
                    text: (parent.done ? 1 : 0) + " / " + (parent.planned ? 1 : 0)
                    color: tableCard.win.text
                    font.family: tableCard.win.mono
                    font.pixelSize: 12
                    horizontalAlignment: Text.AlignRight
                }
                Label {
                    width: parent.width * .16
                    text: tableCard.win.formatMinutes(parent.minutes)
                    color: tableCard.win.text
                    font.family: tableCard.win.mono
                    font.pixelSize: 12
                    horizontalAlignment: Text.AlignRight
                }
                Label {
                    width: parent.width * .15
                    text: String(parent.count || 0)
                    color: tableCard.win.text
                    font.family: tableCard.win.mono
                    font.pixelSize: 12
                    horizontalAlignment: Text.AlignRight
                }
                Label {
                    width: parent.width * .15
                    text: (parent.prayers || 0) + " / 5"
                    color: tableCard.win.text
                    font.family: tableCard.win.mono
                    font.pixelSize: 12
                    horizontalAlignment: Text.AlignRight
                }
            }
        }
        Row {
            width: parent.width
            height: 38
            property var t: backend.statsDaily.shownTotals
            Rectangle {
                anchors.top: parent.top
                width: parent.width
                height: 1
                color: tableCard.win.border
            }
            Label {
                width: parent.width * .34
                text: qsTr("Total")
                color: tableCard.win.text
                font.weight: Font.DemiBold
            }
            Label {
                width: parent.width * .2
                text: (parent.t.done || 0) + " / " + (parent.t.planned || 0)
                color: tableCard.win.text
                font.family: tableCard.win.mono
                horizontalAlignment: Text.AlignRight
            }
            Label {
                width: parent.width * .16
                text: tableCard.win.formatMinutes(parent.t.minutes)
                color: tableCard.win.text
                font.family: tableCard.win.mono
                horizontalAlignment: Text.AlignRight
            }
            Label {
                width: parent.width * .15
                text: parent.t.count || 0
                color: tableCard.win.text
                font.family: tableCard.win.mono
                horizontalAlignment: Text.AlignRight
            }
            Label {
                width: parent.width * .15
                text: (parent.t.prayers || 0) + " / " + (backend.statsDaily.shownCount * 5)
                color: tableCard.win.text
                font.family: tableCard.win.mono
                horizontalAlignment: Text.AlignRight
            }
        }
    }
}
