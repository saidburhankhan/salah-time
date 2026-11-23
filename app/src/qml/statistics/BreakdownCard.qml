import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// A titled list of coloured bars: projects, tags, or blocks.
// rows: [{name, color, value, total, detail}] — the bar is value / total.
Rectangle {
    id: card
    property var win
    property string title: ""
    property string empty: qsTr("Nothing tracked")
    property var rows: []
    Layout.fillWidth: true
    Layout.alignment: Qt.AlignTop
    Layout.preferredHeight: content.implicitHeight + 36
    radius: theme.radius
    color: win.panel
    border.color: win.border

    ColumnLayout {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 18
        spacing: 12
        Label {
            text: card.title
            color: card.win.text
            font.pixelSize: 15
            font.weight: Font.Medium
        }
        Label {
            visible: card.rows.length === 0
            text: card.empty
            color: card.win.muted
        }
        Repeater {
            model: card.rows
            ColumnLayout {
                id: row
                required property var modelData
                Layout.fillWidth: true
                spacing: 5
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Rectangle {
                        width: 10
                        height: 10
                        radius: 5
                        color: row.modelData.color || "#78909c"
                    }
                    Label {
                        Layout.fillWidth: true
                        text: String(row.modelData.name || "")
                        textFormat: Text.PlainText
                        color: card.win.text
                        font.pixelSize: 14
                        elide: Text.ElideRight
                    }
                    Label {
                        text: String(row.modelData.detail || "")
                        textFormat: Text.PlainText
                        color: card.win.muted
                        font.family: card.win.mono
                        font.pixelSize: 12
                    }
                }
                Rectangle {
                    Layout.fillWidth: true
                    height: 6
                    radius: 3
                    color: "#2a2823"
                    Rectangle {
                        height: parent.height
                        radius: 3
                        width: parent.width * Math.min(1, Number(row.modelData.value || 0) / Math.max(1, Number(row.modelData.total || 1)))
                        color: row.modelData.color || theme.accent
                    }
                }
                Label {
                    visible: !!row.modelData.extra
                    Layout.fillWidth: true
                    text: String(row.modelData.extra || "")
                    textFormat: Text.PlainText
                    color: card.win.muted
                    font.pixelSize: 12
                    elide: Text.ElideRight
                }
            }
        }
    }
}
