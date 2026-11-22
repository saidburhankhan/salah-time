import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

RowLayout {
    id: rangeBar
    property var win
    width: parent.width
    spacing: 10
    Rectangle {
        height: 38
        width: 250
        radius: 8
        color: rangeBar.win.panel
        border.color: rangeBar.win.border
        Row {
            anchors.fill: parent
            Repeater {
                model: [["week", qsTr("Week")], ["month", qsTr("Month")], ["custom", qsTr("Custom")]]
                Rectangle {
                    required property var modelData
                    width: parent.parent.width / 3
                    height: parent.parent.height
                    radius: 7
                    color: rangeBar.win.statsMode === modelData[0] ? "#3a3730" : "transparent"
                    Label {
                        anchors.centerIn: parent
                        text: modelData[1]
                        color: rangeBar.win.statsMode === modelData[0] ? rangeBar.win.text : rangeBar.win.muted
                        font.pixelSize: 13
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: {
                            rangeBar.win.statsMode = modelData[0]
                            if (rangeBar.win.statsMode === "custom")
                                rangeBar.win.customRangeVisible = true
                            else
                                rangeBar.win.loadStatistics()
                        }
                    }
                }
            }
        }
    }
    AppButton {
        win: rangeBar.win
        text: "‹"
        implicitWidth: 40
        onClicked: rangeBar.win.stepStats(-1)
    }
    Label {
        Layout.preferredWidth: 210
        text: rangeBar.win.statsRangeLabel()
        color: rangeBar.win.text
        font.pixelSize: 15
        font.weight: Font.Medium
        horizontalAlignment: Text.AlignHCenter
    }
    AppButton {
        win: rangeBar.win
        text: "›"
        implicitWidth: 40
        onClicked: rangeBar.win.stepStats(1)
    }
    Item { Layout.fillWidth: true }
    AppButton {
        win: rangeBar.win
        text: qsTr("Today")
        onClicked: {
            rangeBar.win.statsAnchor = new Date()
            rangeBar.win.loadStatistics()
        }
    }
}
