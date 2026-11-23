import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// Tags group projects across the Statistics filter.
RowLayout {
    id: tags
    property var win
    property string editId: ""
    property string editColor: "#2e7d32"
    property bool confirmingRemove: false
    spacing: 20

    function startNew() {
        editId = ""
        nameField.text = ""
        editColor = "#2e7d32"
        confirmingRemove = false
        nameField.forceActiveFocus()
    }

    function startEdit(tag) {
        editId = String(tag.id)
        nameField.text = String(tag.name || "")
        editColor = String(tag.color || "#2e7d32")
        confirmingRemove = false
    }

    function save() {
        var name = nameField.text.trim()
        if (name === "")
            return
        var fields = { name: name, color: editColor }
        if (editId !== "")
            fields.id = editId
        backend.saveEntity("tag", fields)
    }

    function usage(id) {
        var count = 0
        var list = backend.projects || []
        for (var i = 0; i < list.length; i++) {
            if ((list[i].tag_ids || []).indexOf(id) >= 0)
                count++
        }
        return count
    }

    Component.onCompleted: startNew()

    Connections {
        target: backend
        function onEntitySaved(kind, id) {
            if (kind !== "tag")
                return
            if (id !== "")
                tags.editId = id
            else
                tags.startNew()
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
            AppButton {
                win: tags.win
                text: qsTr("+  New tag")
                primary: true
                onClicked: tags.startNew()
            }
            Item {
                Layout.fillWidth: true
            }
        }

        Label {
            visible: (backend.tags || []).length === 0
            Layout.fillWidth: true
            Layout.topMargin: 12
            text: qsTr("No tags yet. Tags group projects, for example Faith or Health, and filter the statistics.")
            color: tags.win.muted
            wrapMode: Text.WordWrap
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 8
            model: backend.tags || []
            delegate: Card {
                id: tagRow
                required property var modelData
                readonly property int projectCount: tags.usage(String(modelData.id))
                win: tags.win
                width: ListView.view.width
                height: 60
                border.color: tags.editId === String(modelData.id) ? theme.accent : tags.win.border
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    spacing: 12
                    Rectangle {
                        width: 14
                        height: 14
                        radius: 7
                        color: String(tagRow.modelData.color || "#78909c")
                    }
                    Label {
                        Layout.fillWidth: true
                        text: String(tagRow.modelData.name || "")
                        textFormat: Text.PlainText
                        color: tags.win.text
                        font.pixelSize: 15
                        elide: Text.ElideRight
                    }
                    Label {
                        text: tagRow.projectCount === 1 ? qsTr("1 project") : qsTr("%1 projects").arg(tagRow.projectCount)
                        color: tags.win.muted
                        font.pixelSize: 12
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: tags.startEdit(tagRow.modelData)
                }
            }
        }
    }

    Card {
        win: tags.win
        Layout.fillWidth: true
        Layout.alignment: Qt.AlignTop
        Layout.preferredHeight: tagEditor.implicitHeight + 48

        ColumnLayout {
            id: tagEditor
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 24
            spacing: 18

            Label {
                text: tags.editId === "" ? qsTr("New tag") : qsTr("Edit tag")
                color: tags.win.text
                font.pixelSize: 19
                font.weight: Font.DemiBold
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 6
                TextField {
                    id: nameField
                    Layout.fillWidth: true
                    placeholderText: qsTr("Name, e.g. Faith or Health")
                    onAccepted: tags.save()
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8
                Label {
                    text: qsTr("Colour")
                    color: tags.win.muted
                    font.pixelSize: 13
                }
                ColorSwatches {
                    win: tags.win
                    value: tags.editColor
                    onPicked: function(color) { tags.editColor = color }
                }
            }
            Label {
                visible: tags.confirmingRemove
                Layout.fillWidth: true
                text: qsTr("Removing a tag takes it off every project. Sessions are kept.")
                color: "#ef8068"
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 10
                AppButton {
                    win: tags.win
                    visible: tags.editId !== ""
                    text: tags.confirmingRemove ? qsTr("Confirm remove") : qsTr("Remove")
                    onClicked: {
                        if (tags.confirmingRemove)
                            backend.removeTag(tags.editId)
                        else
                            tags.confirmingRemove = true
                    }
                }
                Item {
                    Layout.fillWidth: true
                }
                AppButton {
                    win: tags.win
                    text: qsTr("Cancel")
                    onClicked: tags.startNew()
                }
                AppButton {
                    win: tags.win
                    primary: true
                    enabled: nameField.text.trim() !== ""
                    text: tags.editId === "" ? qsTr("Create tag") : qsTr("Save")
                    onClicked: tags.save()
                }
            }
        }
    }
}
