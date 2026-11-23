import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// A drawer on the right edge with the resources (notes, links, reminders) of
// one task or epic. Non-modal, so the page stays usable beside it.
Popup {
    id: drawer
    objectName: "resourceDrawer"
    required property var win
    property string targetKind: "item"
    property string targetId: ""
    property string targetTitle: ""
    // The id of the resource being edited, "" for none, "new" for the add form.
    property string formFor: ""

    function show(kind, id, title) {
        targetKind = kind
        targetId = id
        targetTitle = title
        formFor = ""
        backend.loadResources(kind, id)
        open()
    }
    function startAdd() { form.load(null); formFor = "new" }
    function startEdit(res) { form.load(res); formFor = String(res.id) }

    parent: Overlay.overlay
    x: win.width - width
    y: 0
    width: Math.min(420, win.width - 80)
    height: win.height
    modal: false
    focus: true
    padding: 0
    closePolicy: Popup.CloseOnEscape
    onClosed: { backend.closeResources(); win.resourceTargetId = "" }
    onOpened: win.resourceTargetId = targetId
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 120 } }
    exit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 90 } }
    background: Rectangle {
        color: "#1a1916"
        border.width: 1
        border.color: drawer.win.border
    }

    Connections {
        target: backend
        function onResourceWriteFinished(ok, message) {
            if (drawer.formFor === "") return
            form.waiting = false
            if (ok) drawer.formFor = ""
            else form.serverError = message
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 12

        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Label {
                    text: qsTr("RESOURCES")
                    color: drawer.win.muted
                    font.family: drawer.win.mono
                    font.pixelSize: 11
                    font.letterSpacing: 1.5
                }
                Label {
                    objectName: "resourceDrawerTitle"
                    Layout.fillWidth: true
                    text: drawer.targetTitle
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: drawer.win.text
                    font.pixelSize: 18
                    font.weight: Font.DemiBold
                }
            }
            AppButton {
                objectName: "resourceAdd"
                win: drawer.win
                text: qsTr("+ Add")
                primary: true
                implicitHeight: 34
                enabled: drawer.formFor === ""
                onClicked: drawer.startAdd()
            }
            AppButton {
                objectName: "resourceClose"
                win: drawer.win
                text: "✕"
                implicitHeight: 34
                onClicked: drawer.close()
            }
        }

        ResourceForm {
            id: form
            objectName: "resourceForm"
            visible: drawer.formFor !== ""
            Layout.fillWidth: true
            win: drawer.win
            onCancelled: drawer.formFor = ""
            onSubmitted: function(fields) {
                if (drawer.formFor === "new") backend.addResource(fields)
                else backend.editResource(drawer.formFor, fields)
            }
        }

        Label {
            visible: backend.resources.length === 0 && drawer.formFor === ""
            Layout.fillWidth: true
            Layout.topMargin: 8
            wrapMode: Text.Wrap
            color: drawer.win.muted
            text: backend.offlineMode ? qsTr("The list needs the daemon. Adding still works.")
                : backend.resourcesLoading ? qsTr("Loading…")
                : qsTr("No notes, links or reminders yet.")
        }

        ScrollView {
            id: scroll
            objectName: "resourceList"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: availableWidth
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            Column {
                width: scroll.availableWidth
                spacing: 8
                Repeater {
                    model: backend.resources
                    ResourceRow {
                        required property var modelData
                        width: parent.width
                        win: drawer.win
                        resource: modelData
                        visible: drawer.formFor !== String(modelData.id)
                        height: visible ? implicitHeight : 0
                        onEditRequested: drawer.startEdit(modelData)
                        onRemoveRequested: backend.removeResource(modelData.id)
                    }
                }
            }
        }
    }
}
