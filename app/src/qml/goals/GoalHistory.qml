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
    readonly property var scored: rows.filter(row => !row.paused)
    readonly property int metCount: scored.filter(row => row.met).length
    readonly property int pausedCount: rows.length - scored.length
    readonly property int streak: {
        var latest = scored.slice().sort((a, b) => String(b.from).localeCompare(String(a.from)))
        var count = 0
        for (var period of latest) {
            if (!period.met) break
            ++count
        }
        return count
    }
    spacing: 4

    function periodText(row) {
        return row.from === row.to ? history.win.shortDate(row.from)
            : history.win.shortDate(row.from) + " – " + history.win.shortDate(row.to)
    }

    Label {
        objectName: "goalHistorySummary"
        Layout.fillWidth: true
        text: rows.length === 0 ? qsTr("No finished or current periods yet.")
            : history.scored.length ? qsTr("Met in %1 of %2 periods").arg(history.metCount).arg(history.scored.length)
            : qsTr("No scored periods.")
        color: history.win.muted
        font.pixelSize: 12
    }
    Label {
        objectName: "goalHistoryPauseSummary"
        visible: history.pausedCount > 0
        Layout.fillWidth: true
        text: (history.pausedCount === 1 ? qsTr("1 paused period") : qsTr("%1 paused periods").arg(history.pausedCount))
            + (history.scored.length ? "  ·  " + qsTr("%1-period streak").arg(history.streak) : "")
        textFormat: Text.PlainText
        color: history.win.muted
        font.pixelSize: 12
    }
    RowLayout {
        visible: history.rows.length > 0
        Layout.fillWidth: true
        spacing: 8
        Repeater {
            model: [qsTr("Period"), qsTr("Value"), qsTr("Target"), qsTr("Result")]
            Label {
                required property string modelData
                required property int index
                Layout.fillWidth: index === 0
                Layout.preferredWidth: index === 0 ? -1 : (index === 3 ? 70 : 96)
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
                color: modelData.paused ? history.win.muted : history.win.text
                font.pixelSize: 12
                elide: Text.ElideRight
            }
            Label {
                Layout.preferredWidth: 96
                horizontalAlignment: Text.AlignRight
                objectName: "goalHistoryValue"
                text: GoalText.progressAmount(history.win, history.goal, modelData)
                textFormat: Text.PlainText
                color: modelData.paused ? history.win.muted : history.win.text
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
                Layout.preferredWidth: 70
                horizontalAlignment: Text.AlignRight
                text: modelData.paused ? qsTr("Paused") : modelData.met ? "✓" : "—"
                color: !modelData.paused && modelData.met ? "#6c9a64" : history.win.muted
                font.pixelSize: 12
            }
        }
    }
}
