import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../goals"

// Projects: a list on the left, the editor on the right. A project groups
// items and sessions and decides which metrics a session asks for.
RowLayout {
    id: projects
    property var win
    property bool showArchived: false
    // The editor edits a copy; "" is a new project.
    property string editId: ""
    property string editColor: "#4c8bf5"
    property var editTags: []
    property var editMetrics: []
    readonly property var currentProject: win.projectById(editId)
    readonly property bool editArchived: !!currentProject && !!currentProject.archived
    property string savingArchiveId: ""
    // A new project follows the metric defaults until the user changes them.
    property bool metricsTouched: false
    readonly property var visibleProjects: (backend.projects || []).filter(function(p) {
        return projects.showArchived || !p.archived
    })
    readonly property var metricChoices: (backend.metrics || []).filter(function(m) {
        return !m.archived
    })
    // Asked metrics in the order they were added; the rest can be added.
    readonly property var askedMetrics: editMetrics.map(function(link) {
        return projects.win.metricById(link.metric_id)
    }).filter(function(m) { return !!m && !m.archived })
    readonly property var otherMetrics: metricChoices.filter(function(m) {
        return projects.metricIndex(String(m.id)) < 0
    })
    spacing: 20

    function startNew() {
        editId = ""
        nameField.text = ""
        noteField.text = ""
        editColor = "#4c8bf5"
        editTags = []
        metricsTouched = false
        editMetrics = defaultMetrics()
        nameField.forceActiveFocus()
    }

    // A new project asks for the built-in ratings. Nothing is required: the
    // done panel's fields are all optional.
    function defaultMetrics() {
        var defaults = []
        for (var i = 0; i < metricChoices.length; i++) {
            if (metricChoices[i].builtin)
                defaults.push({ metric_id: String(metricChoices[i].id), required: false })
        }
        return defaults
    }

    onMetricChoicesChanged: {
        if (editId === "" && !metricsTouched)
            editMetrics = defaultMetrics()
    }

    function startEdit(project) {
        editId = String(project.id)
        nameField.text = String(project.name || "")
        noteField.text = String(project.note || "")
        editColor = String(project.color || "#4c8bf5")
        editTags = (project.tag_ids || []).map(function(t) { return String(t) })
        editMetrics = (project.metrics || []).map(function(link) {
            return { metric_id: String(link.metric_id), required: false }
        })
        metricsTouched = true
    }

    function metricIndex(id) {
        for (var i = 0; i < editMetrics.length; i++) {
            if (editMetrics[i].metric_id === id)
                return i
        }
        return -1
    }

    function toggleAsk(id) {
        metricsTouched = true
        var list = editMetrics.slice()
        var at = metricIndex(id)
        if (at >= 0)
            list.splice(at, 1)
        else
            list.push({ metric_id: id, required: false })
        editMetrics = list
    }

    function toggleTag(id) {
        var list = editTags.slice()
        var at = list.indexOf(id)
        if (at >= 0)
            list.splice(at, 1)
        else
            list.push(id)
        editTags = list
    }

    function save() {
        var name = nameField.text.trim()
        if (name === "")
            return
        // Metrics in the order they were added; `required` is not used.
        var ordered = editMetrics.map(function(m) { return { metric_id: m.metric_id, required: false } })
        var fields = {
            name: name,
            color: editColor,
            note: noteField.text.trim(),
            tag_ids: editTags,
            metrics: ordered
        }
        if (editId !== "")
            fields.id = editId
        backend.saveEntity("project", fields)
    }

    Component.onCompleted: startNew()

    Connections {
        target: backend
        function onProjectArchiveFinished(id, archived, ok, error) {
            if (projects.savingArchiveId !== id) return
            projects.savingArchiveId = ""
            if (!archived || !archiveDialog.visible) return
            if (ok) archiveDialog.close()
            else archiveDialog.errorText = error
        }
        function onEntitySaved(kind, id) {
            if (kind === "project" && id !== "")
                projects.editId = id
        }
    }

    Popup {
        id: archiveDialog
        objectName: "projectArchiveDialog"
        property int request: 0
        property var project: ({})
        property var counts: null
        property string errorText: ""
        readonly property bool saving: projects.savingArchiveId !== ""
        readonly property var current: projects.win.projectById(project.id)
        readonly property bool stale: !current || current.archived || current.version !== project.version

        modal: true
        focus: true
        anchors.centerIn: Overlay.overlay
        width: Math.min(480, projects.win.width - 48)
        padding: 22
        closePolicy: saving ? Popup.NoAutoClose : Popup.CloseOnEscape
        Overlay.modal: Rectangle { color: "#b3000000" }
        background: Card { win: projects.win }

        function preview(row) {
            project = row
            counts = null
            errorText = ""
            open()
            backend.loadProjectPausePreview(++request, project.id)
        }
        Connections {
            target: backend
            function onProjectPausePreviewLoaded(request, counts, error) {
                if (!archiveDialog.visible || request !== archiveDialog.request) return
                archiveDialog.counts = error ? null : counts
                archiveDialog.errorText = error
            }
        }
        contentItem: ColumnLayout {
            spacing: 12
            Label {
                Layout.fillWidth: true
                text: qsTr("Archive %1?").arg(archiveDialog.project.name || "")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: projects.win.text
                font.pixelSize: 20
                font.weight: Font.DemiBold
            }
            Label {
                objectName: "projectPauseCounts"
                Layout.fillWidth: true
                readonly property var counts: archiveDialog.counts
                text: counts ? qsTr("Pauses %1, %2 and %3.")
                    .arg(counts.goals === 1 ? qsTr("1 goal") : qsTr("%1 goals").arg(counts.goals))
                    .arg(counts.routines === 1 ? qsTr("1 routine") : qsTr("%1 routines").arg(counts.routines))
                    .arg(counts.open_tasks === 1 ? qsTr("1 open task") : qsTr("%1 open tasks").arg(counts.open_tasks))
                    : archiveDialog.errorText || qsTr("Loading pause counts…")
                textFormat: Text.PlainText
                wrapMode: Text.WordWrap
                color: projects.win.text
            }
            Label {
                visible: !archiveDialog.saving && archiveDialog.stale
                Layout.fillWidth: true
                text: qsTr("The project changed. Close this dialog and reopen it for current counts.")
                wrapMode: Text.WordWrap
                color: projects.win.muted
            }
            Label {
                visible: !!archiveDialog.counts && !!archiveDialog.errorText
                Layout.fillWidth: true
                text: archiveDialog.errorText
                textFormat: Text.PlainText
                color: projects.win.muted
                wrapMode: Text.WordWrap
            }
            Label {
                Layout.fillWidth: true
                text: qsTr("Past work stays in Statistics.")
                color: projects.win.muted
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                AppButton {
                    win: projects.win
                    text: qsTr("Cancel")
                    enabled: !archiveDialog.saving
                    onClicked: archiveDialog.close()
                }
                AppButton {
                    objectName: "projectArchiveConfirm"
                    win: projects.win
                    primary: true
                    text: qsTr("Archive project")
                    enabled: !!archiveDialog.counts && !archiveDialog.stale && !archiveDialog.saving
                    onClicked: {
                        var project = archiveDialog.project
                        archiveDialog.errorText = ""
                        projects.savingArchiveId = project.id
                        backend.setProjectArchived(project.id, true, project.version ?? -1)
                    }
                }
            }
        }
    }

    ColumnLayout {
        Layout.preferredWidth: 400
        Layout.maximumWidth: 400
        Layout.minimumWidth: 320
        Layout.fillWidth: false
        Layout.fillHeight: true
        spacing: 12

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            AppButton {
                win: projects.win
                text: qsTr("+  New project")
                primary: true
                onClicked: projects.startNew()
            }
            Item {
                Layout.fillWidth: true
            }
            ChoiceChip {
                objectName: "showArchivedProjects"
                win: projects.win
                text: qsTr("Show archived")
                selected: projects.showArchived
                onClicked: projects.showArchived = !projects.showArchived
            }
        }

        Label {
            visible: projects.visibleProjects.length === 0
            Layout.fillWidth: true
            Layout.topMargin: 12
            text: (backend.projects || []).length > 0
                ? qsTr("No active projects. Select Show archived to resume a project.")
                : qsTr("No projects yet. A project groups items and sessions, gives them a colour, and decides what you rate after a session.")
            color: projects.win.muted
            wrapMode: Text.WordWrap
        }

        ListView {
            id: projectList
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 8
            model: projects.visibleProjects
            delegate: Card {
                id: projectRow
                required property var modelData
                objectName: "settingsProject_" + modelData.id
                readonly property bool current: projects.editId === String(modelData.id)
                win: projects.win
                width: ListView.view.width
                height: 74
                border.color: current ? theme.accent : projects.win.border
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    spacing: 12
                    Rectangle {
                        width: 14
                        height: 14
                        radius: 7
                        color: String(projectRow.modelData.color || "#78909c")
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 3
                        Label {
                            Layout.fillWidth: true
                            text: String(projectRow.modelData.name || "")
                            textFormat: Text.PlainText
                            color: projectRow.modelData.archived ? projects.win.muted : projects.win.text
                            font.pixelSize: 15
                            font.weight: Font.Medium
                            elide: Text.ElideRight
                        }
                        Label {
                            Layout.fillWidth: true
                            text: projects.win.projectSummary(projectRow.modelData)
                            textFormat: Text.PlainText
                            color: projects.win.muted
                            font.pixelSize: 12
                            elide: Text.ElideRight
                        }
                    }
                    Label {
                        visible: !!projectRow.modelData.archived
                        text: qsTr("archived")
                        color: projects.win.muted
                        font.family: projects.win.mono
                        font.pixelSize: 11
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: projects.startEdit(projectRow.modelData)
                }
            }
        }
    }

    Card {
        win: projects.win
        Layout.fillWidth: true
        Layout.fillHeight: true

        RowLayout {
            id: editorHeader
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 24
            spacing: 10
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Label {
                    Layout.fillWidth: true
                    text: projects.editId === "" ? qsTr("New project") : qsTr("Edit project")
                    color: projects.win.text
                    font.pixelSize: 19
                    font.weight: Font.DemiBold
                }
                Label {
                    visible: projects.editArchived
                    text: qsTr("Archived · plans paused")
                    color: projects.win.muted
                    font.pixelSize: 12
                }
            }
            AppButton {
                objectName: "projectArchiveAction"
                win: projects.win
                visible: !!projects.currentProject
                enabled: projects.savingArchiveId === ""
                text: projects.editArchived ? qsTr("Resume") : qsTr("Archive")
                onClicked: {
                    var project = projects.currentProject
                    if (project.archived) {
                        projects.savingArchiveId = project.id
                        backend.setProjectArchived(project.id, false, project.version ?? -1)
                    } else {
                        archiveDialog.preview(project)
                    }
                }
            }
            AppButton {
                win: projects.win
                text: qsTr("Cancel")
                onClicked: projects.startNew()
            }
            AppButton {
                objectName: "projectSaveButton"
                win: projects.win
                primary: true
                enabled: nameField.text.trim() !== ""
                text: projects.editId === "" ? qsTr("Create project") : qsTr("Save")
                onClicked: projects.save()
            }
        }
        Rectangle {
            id: editorRule
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: editorHeader.bottom
            anchors.topMargin: 18
            height: 1
            color: projects.win.border
        }

        Flickable {
            id: editorFlick
            objectName: "projectEditorFlick"
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: editorRule.bottom
            anchors.bottom: parent.bottom
            anchors.leftMargin: 24
            anchors.rightMargin: 24
            anchors.topMargin: 8
            anchors.bottomMargin: 16
            clip: true
            contentWidth: width
            contentHeight: editor.implicitHeight + 12
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            ColumnLayout {
                id: editor
                // Room for the first field's floating label.
                y: 12
                width: editorFlick.width - 12
                spacing: 18

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 6
                    TextField {
                        id: nameField
                        objectName: "projectNameField"
                        Layout.fillWidth: true
                        placeholderText: qsTr("Name, e.g. Quran or Study")
                        onAccepted: projects.save()
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Label {
                        text: qsTr("Colour")
                        color: projects.win.muted
                        font.pixelSize: 13
                    }
                    ColorSwatches {
                        win: projects.win
                        value: projects.editColor
                        onPicked: function(color) { projects.editColor = color }
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 6
                    TextField {
                        id: noteField
                        Layout.fillWidth: true
                        placeholderText: qsTr("Note (optional)")
                    }
                }

                ProjectEpics {
                    visible: projects.editId !== ""
                    Layout.fillWidth: true
                    win: projects.win
                    projectId: projects.editId
                }

                GoalsSection {
                    visible: projects.editId !== ""
                    Layout.fillWidth: true
                    win: projects.win
                    scopeKind: "project"
                    scopeId: projects.editId
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Label {
                        text: qsTr("Default tags for new tasks")
                        color: projects.win.muted
                        font.pixelSize: 13
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 8
                        visible: (backend.tags || []).length > 0
                        Repeater {
                            model: backend.tags || []
                            ChoiceChip {
                                required property var modelData
                                win: projects.win
                                text: String(modelData.name || "")
                                dotColor: String(modelData.color || "transparent")
                                selected: projects.editTags.indexOf(String(modelData.id)) >= 0
                                onClicked: projects.toggleTag(String(modelData.id))
                            }
                        }
                    }
                    RowLayout {
                        visible: (backend.tags || []).length === 0
                        spacing: 10
                        Label {
                            text: qsTr("No tags yet.")
                            color: projects.win.muted
                        }
                        AppButton {
                            win: projects.win
                            text: qsTr("Create tags")
                            onClicked: projects.win.openSettings("tags")
                        }
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 6
                    RowLayout {
                        Layout.fillWidth: true
                        Label {
                            Layout.fillWidth: true
                            text: qsTr("The done panel asks for")
                            color: projects.win.muted
                            font.pixelSize: 13
                        }
                        AppButton {
                            win: projects.win
                            text: qsTr("New metric")
                            onClicked: projects.win.openSettings("metrics")
                        }
                    }
                    Label {
                        visible: projects.askedMetrics.length === 0
                        Layout.fillWidth: true
                        text: qsTr("Nothing: marking an item done only takes a note.")
                        color: projects.win.muted
                    }
                    Repeater {
                        model: projects.askedMetrics
                        Rectangle {
                            id: metricRow
                            required property var modelData
                            Layout.fillWidth: true
                            Layout.preferredHeight: 52
                            radius: theme.radius
                            color: projects.win.card
                            border.color: projects.win.border
                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 14
                                anchors.rightMargin: 10
                                spacing: 8
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 2
                                    Label {
                                        Layout.fillWidth: true
                                        text: String(metricRow.modelData.name || "")
                                        textFormat: Text.PlainText
                                        color: projects.win.text
                                        font.pixelSize: 14
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: projects.win.metricKindText(metricRow.modelData)
                                        textFormat: Text.PlainText
                                        color: projects.win.muted
                                        font.family: projects.win.mono
                                        font.pixelSize: 11
                                    }
                                }
                                AppButton {
                                    win: projects.win
                                    text: qsTr("Remove")
                                    onClicked: projects.toggleAsk(String(metricRow.modelData.id))
                                }
                            }
                        }
                    }
                    Flow {
                        visible: projects.otherMetrics.length > 0
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        spacing: 8
                        Repeater {
                            model: projects.otherMetrics
                            ChoiceChip {
                                required property var modelData
                                win: projects.win
                                text: "+ " + String(modelData.name || "")
                                onClicked: projects.toggleAsk(String(modelData.id))
                            }
                        }
                    }
                    Label {
                        Layout.fillWidth: true
                        text: qsTr("Every metric is optional when you mark an item done or add time.")
                        color: projects.win.muted
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                    }
                }
            }
        }
    }
}
