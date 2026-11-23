import QtQuick
import QtQuick.Controls

// The Chart / Table switch in a chart's header.
Row {
    id: modeRow
    property var win
    // false: chart, true: table
    property bool table: false
    spacing: 4
    signal chosen(bool table)

    Repeater {
        model: [
            { label: qsTr("Chart"), table: false },
            { label: qsTr("Table"), table: true }
        ]
        Button {
            id: choice
            required property var modelData
            readonly property bool current: modeRow.table === modelData.table
            objectName: modelData.table ? "chartTableButton" : "chartChartButton"
            text: modelData.label
            implicitHeight: 28
            topInset: 0
            bottomInset: 0
            leftPadding: 10
            rightPadding: 10
            topPadding: 0
            bottomPadding: 0
            contentItem: Text {
                text: choice.text
                textFormat: Text.PlainText
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                color: choice.current ? (modeRow.win ? modeRow.win.selectedText : "#d6d2ca") : (modeRow.win ? modeRow.win.muted : "#8f8a80")
                font.pixelSize: 12
            }
            background: Rectangle {
                radius: 6
                color: choice.current && modeRow.win ? modeRow.win.selectedFill : "transparent"
                border.width: 1
                border.color: modeRow.win ? (choice.current ? modeRow.win.selectedBorder : modeRow.win.border) : "transparent"
            }
            onClicked: modeRow.chosen(modelData.table)
        }
    }
}
