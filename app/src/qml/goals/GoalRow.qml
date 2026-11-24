import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "GoalText.js" as GoalText

// One goal: what it measures, a muted progress bar with a check once met,
// and optional actions (edit target, end, remove, history). Shown in the
// editors' Goals sections and on the Goals page.
Rectangle {
    id: row
    property var win
    property var goal: ({})
    // Scope name for the Goals page; the editors already name their scope.
    property string scopeText: ""
    property bool showActions: true
    property bool showHistoryButton: false
    property bool historyOpen: false
    signal historyToggled()
    readonly property var progress: goal.progress || {}
    readonly property bool met: !!progress.met
    readonly property bool canRemove: !!goal.starts_on && !goal.ends_on && String(goal.starts_on) === String(progress.from)
    property bool editing: false
    property bool confirming: false
    objectName: "goalRow_" + String(goal.id)
    Layout.fillWidth: true
    implicitHeight: column.implicitHeight + 24
    radius: theme.radius
    color: win.card
    border.color: win.border

    function startEdit() {
        confirming = false
        targetField.text = String(goal.target)
        editing = true
        targetField.forceActiveFocus()
    }
    readonly property real editTarget: Number(String(targetField.text).replace(",", "."))
    readonly property bool validTarget: Number.isFinite(editTarget) && editTarget > 0
    function commitEdit() {
        if (!validTarget)
            return
        editing = false
        if (editTarget !== Number(goal.target)) backend.editGoal(String(goal.id), editTarget)
    }

    ColumnLayout {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 12
        spacing: 8
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Label {
                    objectName: "goalTitle"
                    Layout.fillWidth: true
                    text: row.scopeText !== "" ? row.scopeText : GoalText.metricName(row.win, row.goal)
                    textFormat: Text.PlainText
                    color: row.win.text
                    font.pixelSize: 14
                    elide: Text.ElideRight
                }
                Label {
                    objectName: "goalSubtitle"
                    Layout.fillWidth: true
                    text: (row.scopeText !== "" ? GoalText.metricName(row.win, row.goal) + "  ·  " : "")
                        + GoalText.periodText(row.goal, qsTr("Day"), qsTr("Week")) + "  ·  "
                        + GoalText.directionText(row.goal, qsTr("at least"), qsTr("at most")) + " "
                        + GoalText.amount(row.win, row.goal, row.goal.target)
                    textFormat: Text.PlainText
                    color: row.win.muted
                    font.family: row.win.mono
                    font.pixelSize: 11
                    elide: Text.ElideRight
                }
            }
            Label {
                objectName: "goalMet"
                visible: row.met
                text: "✓"
                color: GoalText.tint(row.goal)
                font.pixelSize: 16
            }
            Label {
                objectName: "goalValue"
                text: GoalText.amount(row.win, row.goal, row.progress.value)
                textFormat: Text.PlainText
                color: row.win.text
                font.family: row.win.mono
                font.pixelSize: 13
            }
        }
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 8
            radius: 4
            color: "#2a2823"
            Rectangle {
                objectName: "goalBar"
                width: parent.width * GoalText.ratio(row.goal)
                height: parent.height
                radius: 4
                color: GoalText.tint(row.goal)
                opacity: 0.85
            }
        }
        Label {
            visible: row.progress.from !== undefined && !!row.progress.from
            Layout.fillWidth: true
            text: String(row.progress.from) === String(row.progress.to)
                ? row.win.shortDate(row.progress.from)
                : row.win.shortDate(row.progress.from) + " – " + row.win.shortDate(row.progress.to)
            textFormat: Text.PlainText
            color: row.win.muted
            font.pixelSize: 11
        }
        RowLayout {
            visible: row.showActions
            Layout.fillWidth: true
            spacing: 6
            Item { Layout.fillWidth: true }
            TextField {
                id: targetField
                objectName: "goalTargetField"
                visible: row.editing
                Layout.preferredWidth: 90
                implicitHeight: 36
                inputMethodHints: Qt.ImhFormattedNumbersOnly
                onAccepted: row.commitEdit()
                Keys.onEscapePressed: row.editing = false
            }
            AppButton {
                objectName: "goalEditSave"
                visible: row.editing
                win: row.win
                implicitHeight: 34
                primary: true
                text: qsTr("Save")
                enabled: row.validTarget
                onClicked: row.commitEdit()
            }
            AppButton {
                visible: row.editing
                win: row.win
                implicitHeight: 34
                text: qsTr("Cancel")
                onClicked: row.editing = false
            }
            AppButton {
                objectName: "goalHistoryButton"
                visible: row.showHistoryButton && !row.editing && !row.confirming
                win: row.win
                implicitHeight: 34
                text: row.historyOpen ? qsTr("Hide history") : qsTr("History")
                onClicked: row.historyToggled()
            }
            AppButton {
                objectName: "goalEditButton"
                visible: !row.editing && !row.confirming
                win: row.win
                implicitHeight: 34
                text: qsTr("Edit target")
                onClicked: row.startEdit()
            }
            AppButton {
                objectName: "goalEndButton"
                visible: !row.editing && !row.confirming
                win: row.win
                implicitHeight: 34
                text: qsTr("End")
                onClicked: row.confirming = true
            }
            AppButton {
                objectName: "goalRemoveButton"
                visible: !row.editing && !row.confirming && row.canRemove
                win: row.win
                implicitHeight: 34
                text: qsTr("Remove")
                onClicked: backend.removeGoal(String(row.goal.id))
            }
            AppButton {
                objectName: "goalEndConfirm"
                visible: row.confirming
                win: row.win
                implicitHeight: 34
                text: qsTr("End goal")
                onClicked: {
                    row.confirming = false
                    backend.endGoal(String(row.goal.id))
                }
            }
            AppButton {
                visible: row.confirming
                win: row.win
                implicitHeight: 34
                text: qsTr("Keep")
                onClicked: row.confirming = false
            }
        }
    }
}
