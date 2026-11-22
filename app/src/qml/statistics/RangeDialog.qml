import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

Popup {
    id: customRangePopup
    property var win
    property date shownMonth: win ? win.statsAnchor : new Date()
    property bool pickingEnd: false
    visible: win ? win.customRangeVisible : false
    modal: true
    focus: true
    x: 250
    y: 90
    width: 330
    height: 390
    closePolicy: Popup.CloseOnPressOutside
    onClosed: win.customRangeVisible = false
    background: Rectangle {
        color: customRangePopup.win.panel
        radius: theme.radius
        border.color: customRangePopup.win.border
    }
    Column {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 12
        RowLayout {
            width: parent.width
            AppButton {
                win: customRangePopup.win
                text: "‹"
                onClicked: customRangePopup.shownMonth = new Date(
                    customRangePopup.shownMonth.getFullYear(),
                    customRangePopup.shownMonth.getMonth() - 1, 1)
            }
            Label {
                Layout.fillWidth: true
                text: Qt.locale("en_US").toString(customRangePopup.shownMonth, "MMMM yyyy")
                color: customRangePopup.win.text
                font.pixelSize: 15
                horizontalAlignment: Text.AlignHCenter
            }
            AppButton {
                win: customRangePopup.win
                text: "›"
                onClicked: customRangePopup.shownMonth = new Date(
                    customRangePopup.shownMonth.getFullYear(),
                    customRangePopup.shownMonth.getMonth() + 1, 1)
            }
        }
        Row {
            width: parent.width
            height: 22
            Repeater {
                model: ["M", "T", "W", "T", "F", "S", "S"]
                Label {
                    required property string modelData
                    width: parent.parent.width / 7
                    text: modelData
                    color: customRangePopup.win.muted
                    font.family: customRangePopup.win.mono
                    font.pixelSize: 11
                    horizontalAlignment: Text.AlignHCenter
                }
            }
        }
        Grid {
            width: parent.width
            columns: 7
            rows: 6
            Repeater {
                model: 42
                Rectangle {
                    required property int index
                    width: parent.width / 7
                    height: 38
                    radius: 6
                    property date cellDate: {
                        var f = new Date(customRangePopup.shownMonth.getFullYear(),
                            customRangePopup.shownMonth.getMonth(), 1)
                        var off = (f.getDay() + 6) % 7
                        return new Date(f.getFullYear(), f.getMonth(), 1 - off + index)
                    }
                    property string cellIso: customRangePopup.win.iso(cellDate)
                    property var bounds: customRangePopup.win.statsBounds(
                        "custom", customRangePopup.win.statsAnchor,
                        customRangePopup.win.customFrom, customRangePopup.win.customTo)
                    color: cellIso === bounds.from || cellIso === bounds.to
                        ? theme.accent
                        : (cellIso > bounds.from && cellIso < bounds.to ? "#3a3730" : "transparent")
                    opacity: cellDate.getMonth() === customRangePopup.shownMonth.getMonth() ? 1 : .35
                    Label {
                        anchors.centerIn: parent
                        text: parent.cellDate.getDate()
                        color: parent.cellIso === parent.bounds.from || parent.cellIso === parent.bounds.to
                            ? theme.accentForeground : customRangePopup.win.text
                        font.family: customRangePopup.win.mono
                        font.pixelSize: 12
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: {
                            if (!customRangePopup.pickingEnd) {
                                customRangePopup.win.customFrom = parent.cellDate
                                customRangePopup.win.customTo = parent.cellDate
                                customRangePopup.pickingEnd = true
                            } else {
                                customRangePopup.win.customTo = parent.cellDate
                                customRangePopup.pickingEnd = false
                            }
                        }
                    }
                }
            }
        }
        RowLayout {
            width: parent.width
            Label {
                Layout.fillWidth: true
                text: customRangePopup.win.statsRangeLabel()
                color: customRangePopup.win.muted
                font.family: customRangePopup.win.mono
                font.pixelSize: 11
            }
            AppButton {
                win: customRangePopup.win
                text: qsTr("Apply")
                primary: true
                onClicked: {
                    customRangePopup.win.statsAnchor = customRangePopup.win.customFrom
                    customRangePopup.win.customRangeVisible = false
                    customRangePopup.win.loadStatistics()
                }
            }
        }
    }
}
