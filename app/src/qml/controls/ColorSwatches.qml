import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// A row of colour swatches plus a hex field for anything else.
RowLayout {
    id: swatches
    property var win
    property string value: "#4c8bf5"
    readonly property var colors: ["#4c8bf5", "#2e7d32", "#26a69a", "#c0ca33", "#d4a72c",
                                    "#e0663f", "#e05d8c", "#9b59b6", "#8d6e63", "#78909c"]
    signal picked(string color)
    spacing: 8

    Repeater {
        model: swatches.colors
        Rectangle {
            required property string modelData
            width: 30
            height: 30
            radius: 15
            color: modelData
            border.width: swatches.value.toLowerCase() === modelData ? 3 : 0
            border.color: swatches.win.text
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: swatches.picked(parent.modelData)
            }
        }
    }
    TextField {
        id: hexField
        Layout.preferredWidth: 110
        Layout.leftMargin: 6
        text: swatches.value
        font.family: swatches.win.mono
        placeholderText: qsTr("Hex")
        validator: RegularExpressionValidator { regularExpression: /#[0-9a-fA-F]{0,6}/ }
        onTextEdited: {
            if (/^#[0-9a-fA-F]{6}$/.test(text))
                swatches.picked(text.toLowerCase())
        }
    }
}
