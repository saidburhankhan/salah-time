import QtQuick
import QtQuick.Controls

TextField {
    id: field
    property string minimumDay: ""
    readonly property string day: text.trim()
    readonly property bool valid: /^\d{4}-\d{2}-\d{2}$/.test(day) && day >= minimumDay
        && !isNaN(Date.parse(day)) && new Date(Date.parse(day)).toISOString().slice(0, 10) === day
    inputMask: "0000-00-00"
    inputMethodHints: Qt.ImhDigitsOnly
    placeholderText: qsTr("YYYY-MM-DD")
    font.family: "monospace"
}
