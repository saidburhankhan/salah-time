import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: errorBanner
    property var win
    visible: backend.error !== ""
    Layout.fillWidth: true
    height: 44
    color: "#4a2724"
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 24
        anchors.rightMargin: 18
        Label {
            Layout.fillWidth: true
            text: backend.error
            color: "#ffd5cf"
            textFormat: Text.PlainText
            elide: Text.ElideRight
        }
        ToolButton {
            text: "×"
            onClicked: backend.dismissError()
        }
    }
}
