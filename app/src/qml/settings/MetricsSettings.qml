import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// Metrics: what a session can be rated or counted by. The kind, scale and
// rollup are fixed once a metric exists; built-in metrics are locked.
RowLayout {
    id: metrics
    property var win
    property bool showArchived: false
    property string editId: ""
    property bool editBuiltin: false
    property bool editArchived: false
    property string editKind: "count"
    property int editScaleMax: 5
    property string editRollup: "sum"
    readonly property bool locked: editBuiltin
    readonly property bool existing: editId !== ""
    readonly property var visibleMetrics: (backend.metrics || []).filter(function(m) {
        return metrics.showArchived || !m.archived
    })
    spacing: 20

    function startNew() {
        editId = ""
        editBuiltin = false
        editArchived = false
        nameField.text = ""
        unitField.text = ""
        editKind = "count"
        editScaleMax = 5
        editRollup = "sum"
        nameField.forceActiveFocus()
    }

    function startEdit(metric) {
        editId = String(metric.id)
        editBuiltin = !!metric.builtin
        editArchived = !!metric.archived
        nameField.text = String(metric.name || "")
        unitField.text = String(metric.unit || "")
        editKind = String(metric.kind || "count")
        editScaleMax = Number(metric.scale_max || 5)
        editRollup = String(metric.rollup || "sum")
    }

    function pickKind(kind) {
        editKind = kind
        editRollup = kind === "scale" ? "avg" : "sum"
    }

    function save() {
        var name = nameField.text.trim()
        if (name === "" || locked)
            return
        var fields = { name: name, unit: unitField.text.trim() }
        if (existing) {
            fields.id = editId
        } else {
            fields.kind = editKind
            fields.rollup = editRollup
            if (editKind === "scale")
                fields.scale_max = editScaleMax
        }
        backend.saveEntity("metric", fields)
    }

    Component.onCompleted: startNew()

    Connections {
        target: backend
        function onEntitySaved(kind, id) {
            if (kind === "metric" && id !== "")
                metrics.editId = id
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
                win: metrics.win
                text: qsTr("+  New metric")
                primary: true
                onClicked: metrics.startNew()
            }
            Item {
                Layout.fillWidth: true
            }
            ChoiceChip {
                win: metrics.win
                text: qsTr("Show archived")
                selected: metrics.showArchived
                onClicked: metrics.showArchived = !metrics.showArchived
            }
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 8
            model: metrics.visibleMetrics
            delegate: Card {
                id: metricRow
                required property var modelData
                win: metrics.win
                width: ListView.view.width
                height: 66
                border.color: metrics.editId === String(modelData.id) ? theme.accent : metrics.win.border
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    spacing: 12
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 3
                        Label {
                            Layout.fillWidth: true
                            text: String(metricRow.modelData.name || "")
                            textFormat: Text.PlainText
                            color: metricRow.modelData.archived ? metrics.win.muted : metrics.win.text
                            font.pixelSize: 15
                            elide: Text.ElideRight
                        }
                        Label {
                            text: metrics.win.metricKindText(metricRow.modelData)
                                + "  ·  " + (metricRow.modelData.rollup === "avg" ? qsTr("average") : qsTr("sum"))
                            textFormat: Text.PlainText
                            color: metrics.win.muted
                            font.family: metrics.win.mono
                            font.pixelSize: 11
                        }
                    }
                    Label {
                        visible: !!metricRow.modelData.builtin || !!metricRow.modelData.archived
                        text: metricRow.modelData.builtin ? qsTr("built-in") : qsTr("archived")
                        color: metrics.win.muted
                        font.family: metrics.win.mono
                        font.pixelSize: 11
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: metrics.startEdit(metricRow.modelData)
                }
            }
        }
    }

    Card {
        win: metrics.win
        Layout.fillWidth: true
        Layout.alignment: Qt.AlignTop
        Layout.preferredHeight: metricEditor.implicitHeight + 48

        ColumnLayout {
            id: metricEditor
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 24
            spacing: 18

            Label {
                text: metrics.locked ? qsTr("Built-in metric")
                    : (metrics.existing ? qsTr("Edit metric") : qsTr("New metric"))
                color: metrics.win.text
                font.pixelSize: 19
                font.weight: Font.DemiBold
            }
            Label {
                visible: metrics.locked
                Layout.fillWidth: true
                text: qsTr("Focus and Quality come with Salah and cannot be changed or archived. Choose per project whether they are asked.")
                color: metrics.win.muted
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 14
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 6
                    TextField {
                        id: nameField
                        Layout.fillWidth: true
                        enabled: !metrics.locked
                        placeholderText: qsTr("Name, e.g. Pages read")
                        onAccepted: metrics.save()
                    }
                }
                ColumnLayout {
                    Layout.preferredWidth: 200
                    Layout.fillWidth: false
                    spacing: 6
                    TextField {
                        id: unitField
                        Layout.fillWidth: true
                        enabled: !metrics.locked
                        placeholderText: qsTr("Unit, e.g. pages")
                        onAccepted: metrics.save()
                    }
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8
                Label {
                    text: metrics.existing ? qsTr("Kind (fixed)") : qsTr("Kind")
                    color: metrics.win.muted
                    font.pixelSize: 13
                }
                RowLayout {
                    spacing: 8
                    Repeater {
                        model: [
                            { id: "count", name: qsTr("Count"), hint: qsTr("a number, e.g. pages") },
                            { id: "scale", name: qsTr("Scale"), hint: qsTr("1 to N taps") },
                            { id: "check", name: qsTr("Check"), hint: qsTr("yes or no") }
                        ]
                        ChoiceChip {
                            required property var modelData
                            win: metrics.win
                            text: modelData.name
                            enabled: !metrics.existing
                            selected: metrics.editKind === modelData.id
                            onClicked: metrics.pickKind(modelData.id)
                        }
                    }
                }
            }
            ColumnLayout {
                visible: metrics.editKind === "scale"
                Layout.fillWidth: true
                spacing: 8
                Label {
                    text: qsTr("Scale maximum")
                    color: metrics.win.muted
                    font.pixelSize: 13
                }
                RowLayout {
                    spacing: 6
                    Repeater {
                        model: [2, 3, 4, 5, 6, 7, 8, 9, 10]
                        ChoiceChip {
                            required property int modelData
                            win: metrics.win
                            implicitWidth: 44
                            leftPadding: 0
                            rightPadding: 0
                            text: String(modelData)
                            enabled: !metrics.existing
                            selected: metrics.editScaleMax === modelData
                            onClicked: metrics.editScaleMax = modelData
                        }
                    }
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8
                Label {
                    text: qsTr("Statistics combine days by")
                    color: metrics.win.muted
                    font.pixelSize: 13
                }
                RowLayout {
                    spacing: 8
                    ChoiceChip {
                        win: metrics.win
                        text: qsTr("Sum")
                        enabled: !metrics.existing
                        selected: metrics.editRollup === "sum"
                        onClicked: metrics.editRollup = "sum"
                    }
                    ChoiceChip {
                        win: metrics.win
                        text: qsTr("Average")
                        enabled: !metrics.existing
                        selected: metrics.editRollup === "avg"
                        onClicked: metrics.editRollup = "avg"
                    }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 6
                spacing: 10
                AppButton {
                    win: metrics.win
                    visible: metrics.existing && !metrics.locked
                    text: metrics.editArchived ? qsTr("Restore") : qsTr("Archive")
                    onClicked: {
                        backend.saveEntity("metric", { id: metrics.editId, archived: !metrics.editArchived })
                        metrics.editArchived = !metrics.editArchived
                    }
                }
                Item {
                    Layout.fillWidth: true
                }
                AppButton {
                    win: metrics.win
                    text: qsTr("Cancel")
                    onClicked: metrics.startNew()
                }
                AppButton {
                    win: metrics.win
                    visible: !metrics.locked
                    primary: true
                    enabled: nameField.text.trim() !== ""
                    text: metrics.existing ? qsTr("Save") : qsTr("Create metric")
                    onClicked: metrics.save()
                }
            }
        }
    }
}
