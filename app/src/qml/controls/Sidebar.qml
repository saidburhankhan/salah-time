import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "."

Rectangle {
    id: sidebar
    property var win
    property alias todayLink: todayLink
    Layout.preferredWidth: 220
    Layout.fillHeight: true
    color: "#1c1b18"
    border.color: sidebar.win.border
    border.width: 0
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 8
        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 64
            spacing: 12
            Item {
                width: 28
                height: 28
                Rectangle {
                    width: 24
                    height: 24
                    radius: 12
                    color: sidebar.win.text
                    anchors.centerIn: parent
                }
                Rectangle {
                    width: 20
                    height: 20
                    radius: 10
                    color: "#1c1b18"
                    x: 10
                    y: 0
                }
            }
            Label {
                text: "Salah"
                color: sidebar.win.text
                font.pixelSize: 21
                font.weight: Font.DemiBold
            }
        }
        SidebarItem {
            id: todayLink
            objectName: "todaySidebarItem"
            appWindow: sidebar.win
            pageIndex: 0
            label: qsTr("Today")
            iconKind: "today"
            dropTarget: sidebar.win.backlogDrag.active
        }
        SidebarItem {
            objectName: "backlogSidebarItem"
            appWindow: sidebar.win
            pageIndex: 5
            label: qsTr("Backlog")
            iconKind: "backlog"
        }
        Repeater {
            model: [qsTr("Routines"), qsTr("Statistics")]
            SidebarItem {
                required property string modelData
                required property int index
                appWindow: sidebar.win
                pageIndex: index + 1
                label: modelData
                iconKind: index === 0 ? "routines" : "statistics"
                loadStatsOnClick: index === 1
            }
        }
        SidebarItem {
            objectName: "insightsSidebarItem"
            appWindow: sidebar.win
            pageIndex: 7
            label: qsTr("Insights")
            iconKind: "insights"
        }
        SidebarItem {
            objectName: "goalsSidebarItem"
            appWindow: sidebar.win
            pageIndex: 6
            label: qsTr("Goals")
            iconKind: "goals"
        }
        SidebarItem {
            objectName: "reviewSidebarItem"
            appWindow: sidebar.win
            pageIndex: 8
            label: qsTr("Review")
            iconKind: "review"
        }
        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: sidebar.win.border
            Layout.topMargin: 8
            Layout.bottomMargin: 8
        }
        SidebarItem {
            appWindow: sidebar.win
            pageIndex: 3
            label: qsTr("Settings")
            iconKind: "settings"
        }
        Item { Layout.fillHeight: true }
        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 48
            LineIcon {
                win: sidebar.win
                kind: "location"
                iconColor: sidebar.win.muted
                Layout.leftMargin: 12
            }
            Label {
                text: sidebar.win.locationName()
                color: "#c4c0b8"
                font.pixelSize: 15
                Layout.fillWidth: true
                elide: Text.ElideRight
            }
        }
        Label {
            text: qsTr("Offline mode")
            visible: backend.offlineMode
            color: sidebar.win.muted
            font.pixelSize: 12
            opacity: 0.75
            Layout.leftMargin: 12
            Layout.topMargin: -4
            textFormat: Text.PlainText
        }
    }
    Rectangle {
        anchors.right: parent.right
        width: 1
        height: parent.height
        color: sidebar.win.border
    }
}
