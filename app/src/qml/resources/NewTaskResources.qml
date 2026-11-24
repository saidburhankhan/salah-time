import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// Draft resources have no server owner until the task is saved.
ColumnLayout {
    id: section
    required property var win
    property var entries: []
    property bool composing: false
    spacing: 8
    function reset() { entries = []; composing = false }
    RowLayout {
        Layout.fillWidth: true
        Label {
            Layout.fillWidth: true
            text: qsTr("Notes & links")
            color: section.win.text
            font.weight: Font.Medium
        }
        AppButton {
            objectName: "composeAttachment"
            win: section.win
            visible: !section.composing
            text: qsTr("+ Add")
            onClicked: section.composing = true
        }
    }
    Repeater {
        model: section.entries
        RowLayout {
            required property var modelData
            required property int index
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: modelData.body || modelData.title || modelData.url || modelData.remind_at || qsTr("Note")
                textFormat: Text.PlainText
                color: section.win.text
                elide: Text.ElideRight
            }
            AppButton {
                win: section.win
                text: qsTr("Remove")
                onClicked: section.entries = section.entries.filter((entry, i) => i !== index)
            }
        }
    }
    Loader {
        Layout.fillWidth: true
        active: section.composing
        visible: active
        sourceComponent: ResourceForm {
            objectName: "attachmentComposer"
            win: section.win
            submitLabel: qsTr("Attach")
            Component.onCompleted: load(null)
            onSubmitted: function(fields) {
                section.entries = section.entries.concat([fields])
                section.composing = false
            }
            onCancelled: section.composing = false
        }
    }
}
