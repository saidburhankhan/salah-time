import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "GoalText.js" as GoalText

// A goal's history as a small table: one row per period with the value, the
// target that period had, and whether it was met. The count of met periods
// is shown too, so a short history reads as a count rather than a rate.
ColumnLayout {
    id: history
    objectName: "goalHistory"
    property var win
    property var goal: ({})
    property var rows: []
    readonly property int metCount: rows.filter(function(row) { return row.met }).length
    spacing: 4

    function periodText(row) {
        return row.from === row.to ? history.win.shortDate(row.from)
            : history.win.shortDate(row.from) + " – " + history.win.shortDate(row.to)
    }

    Label {
        objectName: "goalHistorySummary"
        Layout.fillWidth: true
        text: rows.length === 0 ? qsTr("No finished or current periods yet.")
            : qsTr("Met in %1 of %2 periods").arg(history.metCount).arg(history.rows.length)
        color: history.win.muted
        font.pixelSize: 12
    }
    RowLayout {
        visible: history.rows.length > 0
        Layout.fillWidth: true
        spacing: 8
        Repeater {
            model: [qsTr("Period"), qsTr("Value"), qsTr("Target"), qsTr("Met")]
            Label {
                required property string modelData
                required property int index
                Layout.fillWidth: index === 0
                Layout.preferredWidth: index === 0 ? -1 : (index === 3 ? 44 : 96)
                horizontalAlignment: index === 0 ? Text.AlignLeft : Text.AlignRight
                text: modelData
                color: history.win.muted
                font.pixelSize: 11
            }
        }
    }
    Repeater {
        model: history.rows
        RowLayout {
            required property var modelData
            required property int index
            objectName: "goalHistoryRow_" + index
            Layout.fillWidth: true
            spacing: 8
            Label {
                Layout.fillWidth: true
                text: history.periodText(modelData)
                textFormat: Text.PlainText
                color: history.win.text
                font.pixelSize: 12
                elide: Text.ElideRight
            }
            Label {
                Layout.preferredWidth: 96
                horizontalAlignment: Text.AlignRight
                text: GoalText.amount(history.win, history.goal, modelData.value)
                textFormat: Text.PlainText
                color: history.win.text
                font.family: history.win.mono
                font.pixelSize: 12
            }
            Label {
                Layout.preferredWidth: 96
                horizontalAlignment: Text.AlignRight
                text: GoalText.amount(history.win, history.goal, modelData.target)
                textFormat: Text.PlainText
                color: history.win.muted
                font.family: history.win.mono
                font.pixelSize: 12
            }
            Label {
                Layout.preferredWidth: 44
                horizontalAlignment: Text.AlignRight
                text: modelData.met ? "✓" : "—"
                color: modelData.met ? "#6c9a64" : history.win.muted
                font.pixelSize: 12
            }
        }
    }
}
