import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

ScrollView {
    id: entryFields
    property var win
    property var form
    property alias titleText: titleField.text
    property alias goalText: plannedField.text
    property bool creatingProject: false
    Layout.fillWidth: true
    Layout.fillHeight: true
    Layout.minimumWidth: 450
    clip: true
    function initialize(title, target) {
        titleField.text = title
        plannedField.text = target
    }
    function clearTitle() {
        titleField.text = ""
    }
    // Creates a project from the inline field and selects it once saved.
    function createProject() {
        var name = newProjectField.text.trim()
        if (name === "" || creatingProject)
            return
        var palette = ["#4c8bf5", "#2e7d32", "#e0663f", "#9b59b6", "#d4a72c", "#26a69a", "#e05d8c"]
        var color = palette[(backend.projects || []).length % palette.length]
        creatingProject = true
        backend.saveEntity("project", { name: name, color: color })
    }
    Connections {
        target: backend
        function onEntitySaved(kind, id) {
            if (kind !== "project" || !entryFields.creatingProject)
                return
            entryFields.creatingProject = false
            newProjectField.text = ""
            if (id !== "")
                form.selectProject(id)
        }
        function onEntitySaveFailed(kind, error) {
            if (kind === "project")
                entryFields.creatingProject = false
        }
    }
    Column {
        width: entryFields.availableWidth
        spacing: 18
        Rectangle {
            width: parent.width
            height: 108
            radius: theme.radius
            color: form.win.panel
            border.color: form.win.border
            Item {
                anchors.fill: parent
                anchors.margins: 24
                Text {
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    text: qsTr("Activity title…")
                    color: form.win.muted
                    font.pixelSize: 24
                    visible: titleField.text.length === 0
                }
                TextInput {
                    id: titleField
                    objectName: "entryTitleField"
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    color: form.win.text
                    selectionColor: theme.accent
                    selectedTextColor: theme.accentForeground
                    font.pixelSize: 28
                    clip: true
                    onAccepted: form.submitTitle(false)
                }
                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: 1
                    color: form.win.border
                }
            }
        }
        Rectangle {
            visible: !form.backlog
            width: parent.width
            height: 150
            radius: theme.radius
            color: form.win.panel
            border.color: form.win.border
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 22
                spacing: 16
                Label {
                    text: qsTr("When")
                    color: form.win.text
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                }
                RowLayout {
                    spacing: 8
                    Repeater {
                        model: form.win.blockIds
                        ChoiceChip {
                            required property string modelData
                            required property int index
                            win: form.win
                            text: form.win.blockNames[index]
                            selected: form.block === modelData
                            onClicked: form.block = modelData
                        }
                    }
                }
            }
        }
        Rectangle {
            width: parent.width
            height: planColumn.implicitHeight + 44
            radius: theme.radius
            color: form.win.panel
            border.color: form.win.border
            ColumnLayout {
                id: planColumn
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 22
                spacing: 14
                Label {
                    text: qsTr("Project")
                    color: form.win.text
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 8
                    ChoiceChip {
                        objectName: "noProjectChip"
                        win: form.win
                        text: qsTr("No project")
                        selected: form.projectId === ""
                        onClicked: form.selectProject("")
                    }
                    Repeater {
                        model: form.win.activeProjects()
                        ChoiceChip {
                            required property var modelData
                            win: form.win
                            text: String(modelData.name || "")
                            dotColor: String(modelData.color || "transparent")
                            selected: form.projectId === String(modelData.id)
                            onClicked: form.selectProject(String(modelData.id))
                        }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    TextField {
                        id: newProjectField
                        objectName: "newProjectField"
                        Layout.preferredWidth: 240
                        placeholderText: form.win.activeProjects().length === 0
                            ? qsTr("Name your first project")
                            : qsTr("New project name")
                        onAccepted: entryFields.createProject()
                    }
                    AppButton {
                        win: form.win
                        text: qsTr("+  Create project")
                        enabled: newProjectField.text.trim() !== "" && !entryFields.creatingProject
                        onClicked: entryFields.createProject()
                    }
                    Label {
                        Layout.fillWidth: true
                        text: qsTr("Colours and metrics: Settings → Projects")
                        color: form.win.muted
                        font.pixelSize: 12
                        elide: Text.ElideRight
                    }
                }
                ColumnLayout {
                    objectName: "epicChoices"
                    visible: form.epicChoices.length > 0
                    Layout.fillWidth: true
                    Layout.topMargin: 6
                    spacing: 8
                    Label {
                        text: qsTr("Epic")
                        color: form.win.text
                        font.pixelSize: 15
                        font.weight: Font.Medium
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 8
                        ChoiceChip {
                            objectName: "noEpicChip"
                            win: form.win
                            text: qsTr("No epic")
                            selected: form.epicId === ""
                            onClicked: form.epicId = ""
                        }
                        Repeater {
                            model: form.epicChoices
                            ChoiceChip {
                                required property var modelData
                                objectName: "epicChip_" + String(modelData.id)
                                win: form.win
                                text: String(modelData.name || "")
                                selected: form.epicId === String(modelData.id)
                                onClicked: form.epicId = String(modelData.id)
                            }
                        }
                    }
                }
                ColumnLayout {
                    visible: !backend.offlineMode
                    Layout.fillWidth: true
                    Layout.topMargin: 10
                    spacing: 8
                    Label {
                        text: qsTr("Asks")
                        color: form.win.text
                        font.pixelSize: 15
                        font.weight: Font.Medium
                    }
                    Flow {
                        visible: !form.projectChanged
                        Layout.fillWidth: true
                        spacing: 8
                        Repeater {
                            model: form.askChoices
                            ChoiceChip {
                                required property var modelData
                                win: form.win
                                text: String(modelData.name)
                                selected: form.asksMetric(modelData.metric_id)
                                onClicked: form.toggleMetric(modelData.metric_id)
                            }
                        }
                        AppButton {
                            visible: form.metricIds !== null
                            win: form.win
                            text: qsTr("Reset to project")
                            onClicked: form.resetMetrics()
                        }
                    }
                    Label {
                        visible: form.projectChanged || form.askChoices.length === 0
                        text: form.projectChanged ? qsTr("Save the project change to customize its metrics.")
                            : qsTr("No project metrics to ask.")
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: form.win.muted
                    }
                }
                Rectangle {
                    Layout.fillWidth: true
                    height: 1
                    color: form.win.border
                }
                RowLayout {
                    spacing: 8
                    Label {
                        text: qsTr("Planned minutes")
                        color: form.win.muted
                    }
                    TextField {
                        id: plannedField
                        Layout.preferredWidth: 110
                        placeholderText: qsTr("Optional")
                        font.family: form.win.mono
                        inputMethodHints: Qt.ImhDigitsOnly
                        validator: IntValidator { bottom: 1; top: 1440 }
                    }
                    Repeater {
                        model: [15, 30, 60]
                        ChoiceChip {
                            required property int modelData
                            win: form.win
                            implicitHeight: 36
                            text: qsTr("%1 min").arg(modelData)
                            selected: plannedField.text === String(modelData)
                            onClicked: plannedField.text = String(modelData)
                        }
                    }
                }
            }
        }
        Rectangle {
            visible: !form.backlog
            width: parent.width
            height: form.repeat === "weekly" ? 220 : 150
            radius: theme.radius
            color: form.win.panel
            border.color: form.win.border
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 22
                spacing: 12
                Label {
                    text: qsTr("Repeat")
                    color: form.win.text
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                }
                RowLayout {
                    spacing: 8
                    Repeater {
                        model: [{ id: "none", name: qsTr("Once") },
                            { id: "daily", name: qsTr("Daily") },
                            { id: "weekdays", name: qsTr("Weekdays") },
                            { id: "weekly", name: qsTr("Weekly") }]
                        ChoiceChip {
                            required property var modelData
                            win: form.win
                            text: modelData.name
                            selected: modelData.id !== "weekdays"
                                && form.repeat === modelData.id
                            onClicked: form.selectRepeat(modelData.id)
                        }
                    }
                }
                RowLayout {
                    visible: form.repeat === "weekly"
                    spacing: 6
                    Repeater {
                        model: ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
                        ChoiceChip {
                            required property string modelData
                            win: form.win
                            text: form.win.capitalized(modelData)
                            selected: form.days.indexOf(modelData) >= 0
                            onClicked: form.toggleDay(modelData)
                        }
                    }
                }
                Label {
                    visible: form.repeat === "weekly" && form.days.length === 0
                    text: qsTr("Pick at least one day")
                    color: form.win.muted
                    font.pixelSize: 12
                }
            }
        }
    }
}
