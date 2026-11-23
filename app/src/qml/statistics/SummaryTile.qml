import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// A KPI tile: label, big value, one line of context.
Rectangle {
    id: summaryTile
    property var win
    property string label: ""
    property string value: ""
    property string detail: ""
    Layout.fillWidth: true
    Layout.preferredHeight: 116
    radius: theme.radius
    color: win.panel
    border.color: win.border
    Column {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 7
        Label {
            text: summaryTile.label
            color: summaryTile.win.muted
            font.pixelSize: 13
        }
        Label {
            text: summaryTile.value
            textFormat: Text.PlainText
            color: summaryTile.win.text
            font.family: summaryTile.win.mono
            font.pixelSize: 27
            font.weight: Font.DemiBold
        }
        Label {
            width: parent.width
            text: summaryTile.detail
            textFormat: Text.PlainText
            color: summaryTile.win.muted
            font.pixelSize: 12
            elide: Text.ElideRight
        }
    }
}
