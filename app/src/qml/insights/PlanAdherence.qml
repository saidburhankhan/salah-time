import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

ColumnLayout {
    id: plan
    required property var win
    required property var report
    required property var blockLabel
    readonly property var blocks: win.blockIds.concat(["sleep"])
    readonly property var matrix: {
        var result = {}
        for (var cell of report.block_matrix || []) {
            var key = cell.planned_block || "unplanned"
            if (!result[key]) result[key] = {}
            result[key][cell.actual_block] = cell.minutes
        }
        return result
    }
    spacing: 14

    component Heading: Label {
        color: plan.win.text
        font.weight: Font.DemiBold
        font.pixelSize: 16
    }
    component Cell: Label {
        textFormat: Text.PlainText
        color: plan.win.text
        font.pixelSize: 13
        font.features: { "tnum": 1 }
        horizontalAlignment: Text.AlignRight
    }

    Pane {
        Layout.fillWidth: true
        padding: 18
        background: Card { win: plan.win }
        contentItem: ColumnLayout {
            spacing: 12
            Heading { text: qsTr("On-plan rate per task") }
            Label {
                visible: (plan.report.on_plan || []).some(row => row.item_id && row.scored_minutes < row.minutes)
                Layout.fillWidth: true
                text: qsTr("Rates exclude minutes logged while a project was paused.")
                wrapMode: Text.WordWrap
                color: plan.win.muted
                font.pixelSize: 13
            }
            RowLayout {
                Layout.fillWidth: true
                Cell { Layout.fillWidth: true; horizontalAlignment: Text.AlignLeft; text: qsTr("Task"); color: plan.win.muted }
                Cell { Layout.preferredWidth: 100; text: qsTr("Planned block"); color: plan.win.muted }
                Cell { Layout.preferredWidth: 142; text: qsTr("On-plan / scored min"); color: plan.win.muted }
                Cell { Layout.preferredWidth: 68; text: qsTr("Total min"); color: plan.win.muted }
                Cell { Layout.preferredWidth: 68; text: qsTr("Rate"); color: plan.win.muted }
            }
            Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: plan.win.border }
            Repeater {
                model: plan.report.on_plan || []
                RowLayout {
                    id: task
                    required property var modelData
                    Layout.fillWidth: true
                    readonly property var plannedBlocks: modelData.planned_blocks || []
                    readonly property bool planned: !!modelData.planned_block || plannedBlocks.length > 0
                    readonly property real scoredMinutes: modelData.scored_minutes ?? modelData.minutes
                    readonly property bool hasRate: planned && scoredMinutes > 0
                    Cell {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignLeft
                        wrapMode: Text.Wrap
                        text: task.modelData.name || qsTr("Unplanned")
                    }
                    Cell {
                        Layout.preferredWidth: 100
                        text: task.plannedBlocks.length > 1 ? qsTr("Multiple") : plan.blockLabel(task.modelData.planned_block)
                    }
                    Cell {
                        Layout.preferredWidth: 142
                        text: task.hasRate ? task.modelData.on_plan_minutes + " / " + task.scoredMinutes : "—"
                    }
                    Cell { Layout.preferredWidth: 68; text: String(task.modelData.minutes) }
                    Cell {
                        objectName: "onPlanRate_" + (task.modelData.item_id || "unplanned")
                        Layout.preferredWidth: 68
                        color: task.hasRate ? plan.win.selectedText : plan.win.muted
                        text: task.hasRate ? plan.win.numberText(100 * task.modelData.on_plan_minutes / task.scoredMinutes) + "%" : "—"
                    }
                }
            }
            Label {
                visible: !(plan.report.on_plan || []).length
                text: qsTr("No tracked minutes for these filters.")
                color: plan.win.muted
            }
        }
    }
    Pane {
        Layout.fillWidth: true
        padding: 18
        background: Card { win: plan.win }
        contentItem: ColumnLayout {
            spacing: 10
            Heading { text: qsTr("Planned × actual block") }
            Label {
                text: qsTr("Minutes in each actual block, grouped by the planned block.")
                color: plan.win.muted
                font.pixelSize: 13
            }
            component MatrixRow: RowLayout {
                id: matrixRow
                property string plannedBlock: ""
                Layout.fillWidth: true
                spacing: 4
                Cell {
                    Layout.preferredWidth: 92
                    horizontalAlignment: Text.AlignLeft
                    text: matrixRow.plannedBlock ? plan.blockLabel(matrixRow.plannedBlock) : qsTr("Planned")
                    color: matrixRow.plannedBlock ? plan.win.text : plan.win.muted
                }
                Repeater {
                    model: plan.blocks
                    Rectangle {
                        id: cell
                        required property string modelData
                        objectName: "matrix_" + matrixRow.plannedBlock + "_" + modelData
                        readonly property int minutes: (plan.matrix[matrixRow.plannedBlock] || {})[modelData] || 0
                        Layout.fillWidth: true
                        Layout.preferredWidth: 70
                        implicitHeight: 30
                        radius: 4
                        color: minutes ? plan.win.selectedFill : "transparent"
                        Cell {
                            anchors.centerIn: parent
                            text: matrixRow.plannedBlock ? String(cell.minutes) : plan.blockLabel(cell.modelData)
                            color: cell.minutes ? plan.win.selectedText : plan.win.muted
                            font.pixelSize: 12
                        }
                    }
                }
            }
            MatrixRow {}
            Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: plan.win.border }
            Repeater {
                model: plan.blocks.concat(["unplanned"]).filter(block => plan.matrix[block] !== undefined)
                MatrixRow {
                    required property string modelData
                    plannedBlock: modelData
                }
            }
            Label {
                visible: !(plan.report.block_matrix || []).length
                text: qsTr("No tracked minutes for these filters.")
                color: plan.win.muted
            }
        }
    }
}
