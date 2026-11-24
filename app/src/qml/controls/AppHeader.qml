import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "."

Rectangle {
    id: header
    property var win
    Layout.fillWidth: true
    height: 74
    color: "#171612"
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 28
        anchors.rightMargin: 28
        ColumnLayout {
            visible: header.win.page === 0
            Layout.fillWidth: true
            spacing: 2
            RowLayout {
                spacing: 10
                Label {
                    text: (backend.selectedDay === backend.todayDay ? qsTr("Today · ") : "")
                        + backend.hijriHeader(backend.selectedDay)
                    color: header.win.text
                    font.pixelSize: 22
                    font.weight: Font.DemiBold
                }
                Label {
                    text: backend.gregorianHeader(backend.selectedDay)
                    color: header.win.muted
                    font.family: header.win.mono
                    font.pixelSize: 15
                }
            }
        }
        Label {
            visible: header.win.page !== 0
            Layout.fillWidth: true
            text: header.win.page === 1 ? qsTr("Routines")
                : header.win.page === 2 ? qsTr("Statistics")
                : header.win.page === 4 ? qsTr("Epic")
                : header.win.page === 5 ? qsTr("Backlog")
                : header.win.page === 7 ? qsTr("Insights")
                : header.win.page === 8 ? qsTr("Review")
                : header.win.page === 6 ? qsTr("Goals") : qsTr("Settings")
            color: header.win.text
            font.pixelSize: 22
            font.weight: Font.DemiBold
        }
        AppButton {
            win: header.win
            objectName: "addEntryButton"
            visible: header.win.page === 0 || header.win.page === 5
            enabled: header.win.page === 0 || !backend.offlineMode
            text: qsTr("+  Add")
            primary: true
            Layout.alignment: Qt.AlignRight
            onClicked: {
                if (header.win.page === 5)
                    header.win.openBacklogEntry()
                else
                    header.win.openQuick("")
            }
        }
    }
    Rectangle {
        anchors.bottom: parent.bottom
        height: 1
        width: parent.width
        color: header.win.border
    }
}
