import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

ScrollView {
    id: page
    objectName: "routinesPage"
    required property var win
    property string confirmingId: ""
    property double confirmingVersion: -1
    clip: true
    contentWidth: availableWidth
    onVisibleChanged: {
        confirmingId = ""
        backend.setRoutinesOpen(visible)
    }

    function createRoutine() { win.openEntry({repeat: "daily"}) }

    ColumnLayout {
        width: page.availableWidth
        spacing: 14
        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: qsTr("Routines are tasks that repeat in a prayer block on the days you choose.")
                color: page.win.muted
                wrapMode: Text.WordWrap
                font.pixelSize: 14
            }
            AppButton {
                objectName: "newRoutine"
                visible: rows.count > 0
                win: page.win
                text: qsTr("+ New routine")
                primary: true
                onClicked: page.createRoutine()
            }
        }
        Pane {
            visible: rows.count === 0
            Layout.fillWidth: true
            padding: 24
            background: Card { win: page.win }
            contentItem: ColumnLayout {
                spacing: 12
                Label {
                    text: backend.busy ? qsTr("Loading routines…") : qsTr("No routines yet")
                    color: page.win.text
                    font.pixelSize: 18
                }
                Label {
                    Layout.fillWidth: true
                    text: qsTr("Choose a daily or weekly schedule to add a routine to your plan.")
                    color: page.win.muted
                    wrapMode: Text.WordWrap
                }
                AppButton {
                    objectName: "newRoutineEmpty"
                    win: page.win
                    text: qsTr("+ New routine")
                    primary: true
                    onClicked: page.createRoutine()
                }
            }
        }
        Repeater {
            id: rows
            model: backend.routines
            Card {
                id: row
                required property var routine
                required property var doneStrip
                required property int minutes
                required property int doneDays
                readonly property bool confirming: page.confirmingId === routine.id
                objectName: "routineRow_" + routine.id
                win: page.win
                Layout.fillWidth: true
                implicitHeight: content.implicitHeight + 32
                color: hit.containsMouse ? win.card : win.panel
                MouseArea {
                    id: hit
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: if (!row.confirming) page.win.openEntry(row.routine.id)
                }
                ColumnLayout {
                    id: content
                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: 16 }
                    spacing: 8
                    RowLayout {
                        Layout.fillWidth: true
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 4
                            Label {
                                Layout.fillWidth: true
                                text: row.routine.title
                                textFormat: Text.PlainText
                                color: row.win.text
                                font.pixelSize: 17
                                font.weight: Font.Medium
                                wrapMode: Text.Wrap
                            }
                            Label {
                                Layout.fillWidth: true
                                text: row.win.joinedDetail(row.routine.block, row.routine.schedule_text)
                                textFormat: Text.PlainText
                                color: row.win.muted
                                font.pixelSize: 13
                                wrapMode: Text.Wrap
                            }
                        }
                        AppButton {
                            visible: !row.confirming
                            objectName: "routineEdit"
                            win: row.win
                            text: qsTr("Edit")
                            onClicked: page.win.openEntry(row.routine.id)
                        }
                        AppButton {
                            visible: row.confirming
                            win: row.win
                            text: qsTr("Cancel")
                            onClicked: page.confirmingId = ""
                        }
                        AppButton {
                            objectName: "routineRemove"
                            win: row.win
                            text: row.confirming ? qsTr("Confirm removal") : qsTr("Remove")
                            onClicked: {
                                if (row.confirming) {
                                    backend.removeItem(row.routine.id, page.confirmingVersion)
                                    page.confirmingId = ""
                                } else {
                                    page.confirmingVersion = row.routine.version ?? -1
                                    page.confirmingId = row.routine.id
                                }
                            }
                        }
                    }
                    Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: row.win.border }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6
                            Label {
                                text: row.doneStrip.length
                                    ? qsTr("Done days · last %1 days").arg(row.doneStrip.length)
                                    : backend.busy ? qsTr("Loading recent activity…") : qsTr("Recent activity unavailable")
                                textFormat: Text.PlainText
                                color: row.win.muted
                                font.pixelSize: 12
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                Layout.maximumWidth: 420
                                spacing: 4
                                Repeater {
                                    model: row.doneStrip
                                    Rectangle {
                                        required property bool modelData
                                        Layout.fillWidth: true
                                        implicitHeight: 7
                                        radius: 3
                                        color: modelData ? row.win.selectedBorder : row.win.border
                                    }
                                }
                            }
                        }
                        Label {
                            visible: row.doneStrip.length > 0
                            text: (row.doneDays === 1 ? qsTr("1 day done") : qsTr("%1 days done").arg(row.doneDays))
                                + "  ·  " + qsTr("%1 min").arg(row.minutes)
                            textFormat: Text.PlainText
                            color: row.win.text
                            font.pixelSize: 13
                        }
                    }
                    Label {
                        visible: !!row.routine.next_occurrence
                        text: qsTr("Next · %1").arg(row.win.shortDate(row.routine.next_occurrence))
                        textFormat: Text.PlainText
                        color: row.win.muted
                        font.pixelSize: 12
                    }
                }
            }
        }
    }
}
