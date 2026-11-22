import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "."

Rectangle {
    id: item
    property var appWindow
    property int pageIndex: 0
    property string label: ""
    property string iconKind: ""
    property bool loadStatsOnClick: false
    readonly property bool active: appWindow ? appWindow.page === pageIndex : false
    Layout.fillWidth: true
    height: 48
    radius: theme.radius
    color: active ? appWindow.card : "transparent"
    border.color: active ? appWindow.border : "transparent"
    Row {
        anchors.verticalCenter: parent.verticalCenter
        leftPadding: 14
        spacing: 13
        LineIcon {
            win: item.appWindow
            kind: item.iconKind
            iconColor: item.appWindow ? (item.active ? item.appWindow.text : item.appWindow.muted) : "#8f8a80"
        }
        Label {
            text: item.label
            color: item.appWindow && item.active ? item.appWindow.text : "#c4c0b8"
            font.pixelSize: 16
        }
    }
    MouseArea {
        anchors.fill: parent
        onClicked: {
            item.appWindow.page = item.pageIndex
            if (item.loadStatsOnClick)
                item.appWindow.loadStatistics()
        }
    }
}
