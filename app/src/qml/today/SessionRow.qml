import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../notes"

// One of today's sessions under an item: when, how long, what was rated,
// with Edit and Remove. A running session offers Pause instead.
Rectangle {
    id: sessionRow
    property var win
    property var session: ({})
    property var promptMetrics
    property string itemTitle: ""
    property string projectColor: ""
    property bool confirmingRemove: false
    property var savedSession: null
    readonly property var noteSession: savedSession && savedSession.version > session.version ? savedSession : session
    readonly property bool running: !!session.running
    readonly property bool tick: !running && (session.minutes === null || session.minutes === undefined)
    readonly property string valuesText: win.valuesText(session.values)
    readonly property string epicName: win.epicName(session.epic_id)
    readonly property var notes: noteSession.notes || []
    height: 46 + (noteColumn.implicitHeight > 0 ? noteColumn.implicitHeight + 6 : 0)
    radius: theme.radius
    color: "#181714"
    border.color: running ? theme.accent : "transparent"

    function removeSession() {
        if (!confirmingRemove) {
            confirmingRemove = true
            removeButton.forceActiveFocus()
            return
        }
        confirmingRemove = false
        backend.removeSession(String(session.id))
    }

    RowLayout {
        height: 46
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.leftMargin: 14
        anchors.rightMargin: 8
        spacing: 12

        Label {
            Layout.preferredWidth: 104
            text: sessionRow.running ? qsTr("● running")
                : sessionRow.session.started_at
                    ? sessionRow.win.clockTime(sessionRow.session.started_at) + "–"
                        + sessionRow.win.clockTime(sessionRow.session.ended_at)
                    : (sessionRow.tick ? qsTr("done") : qsTr("logged"))
            color: sessionRow.running ? "#ef8068" : sessionRow.win.muted
            font.family: sessionRow.win.mono
            font.pixelSize: 12
        }
        Label {
            Layout.preferredWidth: 64
            text: sessionRow.running
                ? sessionRow.win.runningClock(backend.running.id === sessionRow.session.id ? backend.running : sessionRow.session)
                : (sessionRow.tick ? "✓" : qsTr("%1 min").arg(sessionRow.session.minutes))
            color: sessionRow.win.text
            font.family: sessionRow.win.mono
            font.pixelSize: 13
        }
        Label {
            Layout.fillWidth: true
            text: [sessionRow.epicName, sessionRow.valuesText]
                .filter(Boolean).join("  ·  ")
            textFormat: Text.PlainText
            color: sessionRow.win.muted
            font.pixelSize: 12
            elide: Text.ElideRight
        }
        AppButton {
            visible: sessionRow.running
            win: sessionRow.win
            primary: true
            text: qsTr("Pause")
            onClicked: sessionRow.win.pauseSession(sessionRow.session.id)
        }
        AppButton {
            visible: !sessionRow.running && !sessionRow.confirmingRemove
            win: sessionRow.win
            text: qsTr("Edit")
            onClicked: sessionRow.win.openSessionPrompt({
                mode: "edit",
                tick: sessionRow.tick,
                version: sessionRow.noteSession.version,
                sessionId: String(sessionRow.session.id),
                projectId: String(sessionRow.session.project_id || ""),
                title: sessionRow.itemTitle,
                color: sessionRow.projectColor,
                minutes: sessionRow.session.minutes || 0,
                metrics: sessionRow.promptMetrics,
                values: sessionRow.session.values || {},
                notes: sessionRow.noteSession.notes || [],
                note: String(sessionRow.noteSession.note || "")
            })
        }
        AppButton {
            id: removeButton
            visible: !sessionRow.running
            win: sessionRow.win
            text: sessionRow.confirmingRemove ? qsTr("Remove") : "×"
            focusPolicy: Qt.StrongFocus
            function submitForm() { if (sessionRow.confirmingRemove) sessionRow.removeSession() }
            function cancelForm() { sessionRow.confirmingRemove = false }
            onClicked: sessionRow.removeSession()
        }
        AppButton {
            visible: sessionRow.confirmingRemove
            win: sessionRow.win
            text: qsTr("Keep")
            onClicked: sessionRow.confirmingRemove = false
        }
    }

    ColumnLayout {
        id: noteColumn
        spacing: 4
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: 46
        anchors.leftMargin: 14
        anchors.rightMargin: 12
        InlineNote {
            win: sessionRow.win
            session: sessionRow.noteSession
            onSaved: session => sessionRow.savedSession = session
        }
        Repeater {
            model: sessionRow.notes
            InlineNote {
                required property var modelData
                win: sessionRow.win
                session: sessionRow.noteSession
                entry: modelData
                onSaved: session => sessionRow.savedSession = session
            }
        }
    }
}
