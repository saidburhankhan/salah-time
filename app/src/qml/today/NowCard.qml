import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

Card {
    id: nowCard
    property var win
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 22
        spacing: 8
        RowLayout {
            Layout.fillWidth: true
            Label {
                text: qsTr("Now")
                color: nowCard.win.text
                font.pixelSize: 18
                font.weight: Font.DemiBold
            }
            Label {
                Layout.fillWidth: true
                text: (backend.dayPlan.now || {}).block === "night"
                    ? qsTr("Before Fajr")
                    : ((backend.dayPlan.now || {}).block
                        ? nowCard.win.capitalized((backend.dayPlan.now || {}).block)
                        : "")
                color: nowCard.win.muted
                font.family: nowCard.win.mono
                horizontalAlignment: Text.AlignRight
            }
        }
        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: nowCard.win.border
            Layout.topMargin: 6
            Layout.bottomMargin: 18
        }
        Label {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("NEXT · %1").arg(((backend.dayPlan.now || {}).next || {}).name || "—")
            color: nowCard.win.muted
            font.family: nowCard.win.mono
            font.pixelSize: 13
            font.letterSpacing: 1
        }
        Label {
            Layout.alignment: Qt.AlignHCenter
            text: Math.floor(nowCard.win.countdown / 3600) + "h "
                + String(Math.floor((nowCard.win.countdown % 3600) / 60)).padStart(2, "0") + "m"
            color: nowCard.win.text
            font.pixelSize: 34
            font.weight: Font.Medium
        }
        Label {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("at %1").arg(((backend.dayPlan.now || {}).next || {}).time || "--:--")
            color: nowCard.win.muted
            font.family: nowCard.win.mono
            font.pixelSize: 14
        }
    }
}
