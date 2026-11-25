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
    property string resumingId: ""
    property string resumingProject: ""
    clip: true
    contentWidth: availableWidth
    onVisibleChanged: {
        confirmingId = ""
        backend.setRoutinesOpen(visible)
    }

    Connections {
        target: backend
        function onProjectArchiveFinished(id, archived, ok, error) {
            if (!archived && id === page.resumingProject) page.resumingProject = ""
        }
        function onRoutineEditFinished(id, ok, error) {
            if (id === page.resumingId) page.resumingId = ""
        }
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
                visible: rows.count > 0 || endedRows.count > 0 || pausedRows.count > 0
                win: page.win
                text: qsTr("+ New routine")
                primary: true
                onClicked: page.createRoutine()
            }
        }
        Pane {
            visible: rows.count === 0 && endedRows.count === 0 && pausedRows.count === 0
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
            RoutineRow {}
        }
        Label {
            visible: endedRows.count > 0
            Layout.fillWidth: true
            text: qsTr("Ended")
            color: page.win.text
            font.pixelSize: 18
            font.weight: Font.DemiBold
        }
        Repeater {
            id: endedRows
            model: backend.endedRoutines
            RoutineRow {}
        }
        Label {
            visible: pausedRows.count > 0
            Layout.fillWidth: true
            text: qsTr("Paused — project archived")
            color: page.win.text
            font.pixelSize: 18
            font.weight: Font.DemiBold
        }
        Repeater {
            id: pausedRows
            model: backend.pausedRoutines
            RoutineRow {}
        }
    }

    component RoutineRow: Card {
        id: row
        required property var routine
        required property var doneStrip
        required property real minutes
        required property int doneDays
        readonly property bool ended: !!routine.ended
        readonly property bool paused: !!routine.paused
        readonly property var project: paused ? win.projectById(routine.project_id) : null
        readonly property bool confirming: page.confirmingId === routine.id
        objectName: "routineRow_" + routine.id
        win: page.win
        Layout.fillWidth: true
        implicitHeight: content.implicitHeight + 32
        color: hit.containsMouse ? win.card : win.panel
        MouseArea {
            id: hit
            enabled: !row.paused
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
                        text: [row.win.joinedDetail(row.routine.block, row.routine.schedule_text),
                               row.project ? row.project.name : ""].filter(Boolean).join(" · ")
                        textFormat: Text.PlainText
                        color: row.win.muted
                        font.pixelSize: 13
                        wrapMode: Text.Wrap
                    }
                }
                AppButton {
                    visible: !row.paused && !row.confirming && !row.ended
                    objectName: "routineEdit"
                    win: row.win
                    text: qsTr("Edit")
                    onClicked: page.win.openEntry(row.routine.id)
                }
                AppButton {
                    visible: !row.paused && !row.confirming && !row.ended
                    objectName: "routineEnd"
                    win: row.win
                    text: row.routine.end_day ? qsTr("Change end date") : qsTr("End routine")
                    onClicked: page.win.openRoutineEnd(row.routine)
                }
                AppButton {
                    visible: !row.paused && !row.confirming && row.ended
                    objectName: "routineResume"
                    win: row.win
                    text: qsTr("Resume")
                    enabled: page.resumingId === ""
                    onClicked: {
                        page.resumingId = row.routine.id
                        backend.setRoutineEndDay(row.routine.id, "", row.routine.version ?? -1)
                    }
                }
                AppButton {
                    visible: !row.paused && row.confirming
                    win: row.win
                    text: qsTr("Cancel")
                    onClicked: page.confirmingId = ""
                }
                AppButton {
                    objectName: "routineUnarchiveProject"
                    visible: row.paused
                    enabled: !!row.project && row.project.archived && page.resumingProject === ""
                    win: row.win
                    text: qsTr("Unarchive project")
                    onClicked: {
                        page.resumingProject = row.project.id
                        backend.setProjectArchived(row.project.id, false, row.project.version ?? -1)
                    }
                }
                AppButton {
                    visible: !row.paused
                    objectName: "routineDelete"
                    win: row.win
                    text: row.confirming ? qsTr("Confirm delete") : qsTr("Delete")
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
            Label {
                visible: !row.paused && row.confirming
                Layout.fillWidth: true
                text: row.ended ? qsTr("Delete this ended routine?")
                    : qsTr("Choose End routine to stop future occurrences and keep the routine in Ended.")
                color: row.win.muted
                wrapMode: Text.WordWrap
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
                visible: !row.paused && !row.ended && !!row.routine.next_occurrence
                text: qsTr("Next · %1").arg(row.win.shortDate(row.routine.next_occurrence))
                textFormat: Text.PlainText
                color: row.win.muted
                font.pixelSize: 12
            }
            Label {
                visible: !!row.routine.end_day
                text: (row.ended ? qsTr("Ended · %1") : qsTr("Last day · %1"))
                    .arg(row.win.shortDate(row.routine.end_day))
                textFormat: Text.PlainText
                color: row.win.muted
                font.pixelSize: 12
            }
        }
    }
}
