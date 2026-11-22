import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: ghost
    property var win
    visible: win.dragging
    z: 1000
    x: 248
    width: Math.max(360, win.width - 248 - 375)
    height: 72
    y: Math.max(76, Math.min(win.height - height - 8, win.dragPointerY - height / 2))
    radius: theme.radius
    color: "#292722"
    border.color: "#70695d"
    scale: 1.02
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 14
        anchors.rightMargin: 16
        spacing: 12
        Label {
            text: "⠿"
            color: ghost.win.muted
            font.pixelSize: 19
        }
        Rectangle {
            width: 24
            height: 24
            radius: Math.min(5, theme.radius)
            color: ghost.win.dragDone ? theme.accent : "transparent"
            border.color: "#777168"
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 4
            Label {
                text: ghost.win.dragTitle
                color: ghost.win.text
                font.pixelSize: 15
                font.weight: Font.Medium
                Layout.fillWidth: true
                elide: Text.ElideRight
                textFormat: Text.PlainText
            }
            Label {
                text: ghost.win.dragDetail
                color: ghost.win.muted
                font.family: ghost.win.mono
                font.pixelSize: 12
            }
        }
    }
}
