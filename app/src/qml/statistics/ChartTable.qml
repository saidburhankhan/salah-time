import QtQuick
import QtQuick.Controls

// The numbers behind a chart: a header row and one row per day, newest first.
// `columns` is a list of titles; each row of `rows` is a list of strings.
Item {
    id: table
    property var win
    property var columns: []
    property var rows: []
    readonly property real firstWidth: 118

    function columnWidth(index) {
        if (index === 0)
            return firstWidth
        return (width - firstWidth - 14) / Math.max(1, columns.length - 1)
    }

    Row {
        id: head
        width: table.width
        height: 24
        Repeater {
            model: table.columns
            Label {
                required property var modelData
                required property int index
                width: table.columnWidth(index)
                height: head.height
                verticalAlignment: Text.AlignVCenter
                horizontalAlignment: index === 0 ? Text.AlignLeft : Text.AlignRight
                text: String(modelData)
                elide: Text.ElideRight
                textFormat: Text.PlainText
                color: table.win.muted
                font.family: table.win.mono
                font.pixelSize: 11
            }
        }
    }
    Rectangle {
        id: rule
        anchors.top: head.bottom
        width: table.width
        height: 1
        color: table.win.border
    }
    ListView {
        id: list
        anchors.top: rule.bottom
        anchors.bottom: parent.bottom
        width: table.width
        clip: true
        model: table.rows
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}
        delegate: Row {
            id: line
            required property var modelData
            width: list.width
            height: 25
            Repeater {
                model: line.modelData
                Label {
                    required property var modelData
                    required property int index
                    width: table.columnWidth(index)
                    height: line.height
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: index === 0 ? Text.AlignLeft : Text.AlignRight
                    text: String(modelData)
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                    color: index === 0 ? table.win.muted : table.win.text
                    font.family: table.win.mono
                    font.pixelSize: 12
                }
            }
        }
    }
    Label {
        visible: table.rows.length === 0
        anchors.centerIn: parent
        text: qsTr("No days in this range")
        color: table.win.muted
    }
}
