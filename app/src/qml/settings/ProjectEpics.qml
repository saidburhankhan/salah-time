import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// A project's epics inside the project editor: add, rename, finish or reopen,
// and remove an epic that has no sessions (otherwise it is finished). Epics
// are optional; a project works the same without any.
ColumnLayout {
    id: epics
    objectName: "projectEpics"
    property var win
    property string projectId: ""
    // The epic being renamed and the one asking to be removed ("" for none).
    property string renameId: ""
    property string removeId: ""
    readonly property var rows: win.projectEpics(projectId, true)
    spacing: 8

    function add() {
        var name = addField.text.trim()
        if (name === "" || projectId === "")
            return
        backend.saveEntity("epic", { project_id: projectId, name: name })
        addField.text = ""
    }
    function startRename(epic) {
        removeId = ""
        renameId = String(epic.id)
        renameField.text = String(epic.name || "")
        renameField.forceActiveFocus()
    }
    function commitRename() {
        var name = renameField.text.trim()
        if (name !== "" && renameId !== "")
            backend.saveEntity("epic", { id: renameId, name: name })
        renameId = ""
    }
    function setStatus(epic, status) {
        backend.saveEntity("epic", { id: String(epic.id), status: status })
    }
    function summary(epic) {
        var sessions = Number(epic.sessions || 0)
        var parts = []
        if (epic.status === "finished")
            parts.push(qsTr("finished"))
        parts.push(win.durationText(epic.total_minutes))
        parts.push(sessions === 1 ? qsTr("1 session") : qsTr("%1 sessions").arg(sessions))
        return parts.join("  ·  ")
    }
    onProjectIdChanged: {
        renameId = ""
        removeId = ""
    }

    Label {
        text: qsTr("Epics")
        color: epics.win.muted
        font.pixelSize: 13
    }
    Label {
        visible: epics.rows.length === 0
        Layout.fillWidth: true
        text: qsTr("No epics. An epic is an optional goal inside this project; tasks and sessions can stay without one.")
        color: epics.win.muted
        font.pixelSize: 12
        wrapMode: Text.WordWrap
    }
    Repeater {
        model: epics.rows
        Rectangle {
            id: epicRow
            required property var modelData
            readonly property bool finished: modelData.status === "finished"
            readonly property bool renaming: epics.renameId === String(modelData.id)
            readonly property bool hasSessions: Number(modelData.sessions || 0) > 0
            objectName: "epicRow_" + String(modelData.id)
            Layout.fillWidth: true
            Layout.preferredHeight: 56
            radius: theme.radius
            color: epics.win.card
            border.color: epics.win.border
            MouseArea {
                anchors.fill: parent
                enabled: !epicRow.renaming
                cursorShape: Qt.PointingHandCursor
                onClicked: epics.win.openEpic(String(epicRow.modelData.id))
            }
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 14
                anchors.rightMargin: 10
                spacing: 8
                TextField {
                    id: renameField
                    objectName: "epicRenameField"
                    visible: epicRow.renaming
                    Layout.fillWidth: true
                    onAccepted: epics.commitRename()
                    Keys.onEscapePressed: epics.renameId = ""
                }
                ColumnLayout {
                    visible: !epicRow.renaming
                    Layout.fillWidth: true
                    spacing: 2
                    Label {
                        Layout.fillWidth: true
                        text: String(epicRow.modelData.name || "")
                        textFormat: Text.PlainText
                        color: epicRow.finished ? epics.win.muted : epics.win.text
                        font.pixelSize: 14
                        elide: Text.ElideRight
                    }
                    Label {
                        Layout.fillWidth: true
                        text: epics.summary(epicRow.modelData)
                        textFormat: Text.PlainText
                        color: epics.win.muted
                        font.family: epics.win.mono
                        font.pixelSize: 11
                        elide: Text.ElideRight
                    }
                }
                AppButton {
                    visible: epicRow.renaming
                    win: epics.win
                    text: qsTr("Save")
                    primary: true
                    onClicked: epics.commitRename()
                }
                AppButton {
                    visible: !epicRow.renaming && epics.removeId !== String(epicRow.modelData.id)
                    win: epics.win
                    text: qsTr("Open")
                    onClicked: epics.win.openEpic(String(epicRow.modelData.id))
                }
                AppButton {
                    objectName: "epicRenameButton"
                    visible: !epicRow.renaming && epics.removeId !== String(epicRow.modelData.id)
                    win: epics.win
                    text: qsTr("Rename")
                    onClicked: epics.startRename(epicRow.modelData)
                }
                AppButton {
                    objectName: "epicStatusButton"
                    visible: !epicRow.renaming && epics.removeId !== String(epicRow.modelData.id)
                    win: epics.win
                    text: epicRow.finished ? qsTr("Reopen") : qsTr("Finish")
                    onClicked: epics.setStatus(epicRow.modelData, epicRow.finished ? "active" : "finished")
                }
                // Only an epic without sessions can be removed; otherwise finish it.
                AppButton {
                    objectName: "epicRemoveButton"
                    visible: !epicRow.renaming && !epicRow.hasSessions && epics.removeId !== String(epicRow.modelData.id)
                    win: epics.win
                    text: qsTr("Remove")
                    onClicked: epics.removeId = String(epicRow.modelData.id)
                }
                AppButton {
                    objectName: "epicRemoveConfirm"
                    visible: epics.removeId === String(epicRow.modelData.id)
                    win: epics.win
                    text: qsTr("Remove epic")
                    onClicked: {
                        backend.removeEpic(String(epicRow.modelData.id))
                        epics.removeId = ""
                    }
                }
                AppButton {
                    visible: epics.removeId === String(epicRow.modelData.id)
                    win: epics.win
                    text: qsTr("Keep")
                    onClicked: epics.removeId = ""
                }
            }
        }
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        TextField {
            id: addField
            objectName: "epicNameField"
            Layout.fillWidth: true
            placeholderText: qsTr("New epic name")
            onAccepted: epics.add()
        }
        AppButton {
            objectName: "epicAddButton"
            win: epics.win
            text: qsTr("Add epic")
            enabled: addField.text.trim() !== ""
            onClicked: epics.add()
        }
    }
}
