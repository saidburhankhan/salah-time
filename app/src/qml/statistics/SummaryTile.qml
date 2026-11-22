import QtQuick
import QtQuick.Controls

Rectangle {
    id: summaryTile
    property var tileData
    property var win
    width: (parent.width - 36) / 4
    height: 116
    radius: theme.radius
    color: win.panel
    border.color: win.border
    Column {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 7
        Label {
            text: summaryTile.tileData[0]
            color: summaryTile.win.muted
            font.pixelSize: 13
        }
        Label {
            text: summaryTile.tileData[1]
            color: summaryTile.win.text
            font.family: summaryTile.win.mono
            font.pixelSize: 27
            font.weight: Font.DemiBold
        }
        Label {
            text: summaryTile.tileData[2]
            color: summaryTile.win.muted
            font.pixelSize: 12
        }
    }
}
