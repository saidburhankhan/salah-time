import QtQuick
import QtQuick.Controls

Rectangle {
    id: toast
    property var win
    property string text: ""
    visible: text !== ""
    z: 2000
    anchors.horizontalCenter: parent.horizontalCenter
    anchors.bottom: parent.bottom
    anchors.bottomMargin: 28
    width: toastLabel.implicitWidth + 36
    height: 48
    radius: theme.radius
    color: "#302e28"
    border.color: win ? win.border : "#34312a"
    function show(value) {
        text = value
        toastTimer.restart()
    }
    Label {
        id: toastLabel
        anchors.centerIn: parent
        text: toast.text
        color: toast.win.text
        textFormat: Text.PlainText
    }
    Timer {
        id: toastTimer
        interval: 3500
        onTriggered: toast.text = ""
    }
}
