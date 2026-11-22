import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../today"

RowLayout {
    id: todayPage
    property var win
    property alias timelineModel: timeline.timelineModel
    property alias dragging: timeline.dragging
    property alias dragId: timeline.dragId
    property alias dragPayload: timeline.dragPayload
    property alias dragPointerY: timeline.dragPointerY
    function beginDrag(rowIndex, id, globalY) { timeline.beginDrag(rowIndex, id, globalY) }
    function updateDrag(globalY) { timeline.updateDrag(globalY) }
    function finishDrag(cancelled) { timeline.finishDrag(cancelled) }
    function syncTimeline() { timeline.syncTimeline() }

    spacing: 26
    Timeline {
        id: timeline
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.minimumWidth: 360
        win: todayPage.win
    }
    ColumnLayout {
        Layout.preferredWidth: 300
        Layout.minimumWidth: 270
        Layout.maximumWidth: 320
        Layout.fillHeight: true
        spacing: 18
        NowCard {
            Layout.fillWidth: true
            height: 210
            win: todayPage.win
        }
        MonthCalendar {
            Layout.fillWidth: true
            height: 360
            win: todayPage.win
        }
        Item { Layout.fillHeight: true }
    }
}
