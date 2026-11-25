import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// Adds a goal for one scope: period, minutes or a metric, direction for a
// metric, and the target. Nothing is required except a target above zero.
ColumnLayout {
    id: form
    function submitForm() { submit() }
    function cancelForm() { reset(); cancelled() }
    objectName: "goalForm"
    property var win
    // At most one of these is set; none means all work.
    property string projectId: ""
    property string epicId: ""
    property string itemId: ""
    // The metrics a goal here may use (the scope's asked metrics, or the
    // built-ins for all work).
    property var metricChoices: []
    property string period: "day"
    property string metricId: ""
    property string direction: "at_least"
    property string chosenName: ""
    readonly property bool metricAvailable: metricId === "" || metricChoices.some(metric => String(metric.id) === metricId)
    readonly property bool canSubmit: metricAvailable && Number.isFinite(targetValue()) && targetValue() > 0
    signal added()
    signal cancelled()
    spacing: 8

    function reset() {
        period = "day"
        metricId = ""
        direction = "at_least"
        targetField.text = ""
    }
    function targetValue() { return Number(String(targetField.text).replace(",", ".")) }
    function submit() {
        var value = targetValue()
        if (!canSubmit)
            return
        var fields = { period: period, target: value }
        if (itemId !== "") fields.item_id = itemId
        else if (epicId !== "") fields.epic_id = epicId
        else if (projectId !== "") fields.project_id = projectId
        if (metricId !== "") {
            fields.metric_id = metricId
            fields.direction = direction
        }
        backend.addGoal(fields)
        reset()
        added()
    }
    Flow {
        Layout.fillWidth: true
        spacing: 8
        ChoiceChip {
            objectName: "goalPeriod_day"
            win: form.win
            implicitHeight: 34
            text: qsTr("Day")
            selected: form.period === "day"
            onClicked: form.period = "day"
        }
        ChoiceChip {
            objectName: "goalPeriod_week"
            win: form.win
            implicitHeight: 34
            text: qsTr("Week")
            selected: form.period === "week"
            onClicked: form.period = "week"
        }
    }
    Flow {
        Layout.fillWidth: true
        spacing: 8
        ChoiceChip {
            objectName: "goalWhat_minutes"
            win: form.win
            implicitHeight: 34
            text: qsTr("Minutes")
            selected: form.metricId === ""
            onClicked: form.metricId = ""
        }
        Repeater {
            model: form.metricChoices
            ChoiceChip {
                required property var modelData
                objectName: "goalWhat_" + String(modelData.id)
                win: form.win
                implicitHeight: 34
                text: String(modelData.name || "")
                selected: form.metricId === String(modelData.id)
                onClicked: {
                    form.chosenName = String(modelData.name || modelData.id)
                    form.metricId = String(modelData.id)
                }
            }
        }
    }
    Label {
        visible: !form.metricAvailable
        Layout.fillWidth: true
        text: qsTr("%1 is unavailable. Choose another metric or Minutes.").arg(form.chosenName || form.metricId)
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        color: form.win.muted
    }
    Flow {
        visible: form.metricId !== ""
        Layout.fillWidth: true
        spacing: 8
        ChoiceChip {
            objectName: "goalDirection_at_least"
            win: form.win
            implicitHeight: 34
            text: qsTr("At least")
            selected: form.direction === "at_least"
            onClicked: form.direction = "at_least"
        }
        ChoiceChip {
            objectName: "goalDirection_at_most"
            win: form.win
            implicitHeight: 34
            text: qsTr("At most")
            selected: form.direction === "at_most"
            onClicked: form.direction = "at_most"
        }
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        TextField {
            id: targetField
            objectName: "goalNewTarget"
            Layout.fillWidth: true
            placeholderText: form.metricId === "" ? qsTr("Target in minutes") : qsTr("Target")
            inputMethodHints: Qt.ImhFormattedNumbersOnly
            onAccepted: form.submit()
        }
        AppButton {
            objectName: "goalAddButton"
            win: form.win
            primary: true
            text: qsTr("Add goal")
            enabled: form.canSubmit
            onClicked: form.submit()
        }
    }
}
