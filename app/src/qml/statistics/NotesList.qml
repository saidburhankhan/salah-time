import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// The Notes tab: sessions.list for the shared filter, newest first, with a
// text search and a "with notes only" switch (on by default). "Load more"
// follows the cursor; a row opens the session editor.
ColumnLayout {
    id: notes
    objectName: "notesList"
    property var win
    property var lastQuery: ({})
    property bool withNote: true
    property string search: ""
    readonly property var rows: backend.sessionsList || []
    spacing: 10

    function load(query) {
        lastQuery = query
        var q = {}
        for (var key in query)
            q[key] = query[key]
        q.with_note = withNote
        if (search.trim() !== "")
            q.q = search.trim()
        backend.loadSessions(q)
    }
    function reload() {
        load(lastQuery)
    }
    function setSearch(text) {
        search = text
        reload()
    }
    function openSession(session) {
        var project = session.project || null
        var item = session.item || null
        win.openSessionPrompt({
            mode: "edit",
            tick: session.minutes === null || session.minutes === undefined,
            sessionId: String(session.id),
            itemId: String(session.item_id || ""),
            projectId: String(session.project_id || ""),
            title: item ? String(item.title || "") : (project ? String(project.name || "") : qsTr("Session")),
            color: project ? String(project.color || "") : "",
            minutes: session.minutes || 0,
            values: session.values || {},
            notes: session.notes || [],
            note: String(session.note || "")
        })
    }

    Timer {
        id: searchDelay
        interval: 300
        onTriggered: notes.setSearch(searchField.text)
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 12
        TextField {
            id: searchField
            objectName: "notesSearchField"
            Layout.preferredWidth: 320
            placeholderText: qsTr("Search notes")
            onTextEdited: searchDelay.restart()
            onAccepted: {
                searchDelay.stop()
                notes.setSearch(text)
            }
        }
        ChoiceChip {
            objectName: "notesWithNoteSwitch"
            win: notes.win
            text: qsTr("With notes only")
            selected: notes.withNote
            onClicked: {
                notes.withNote = !notes.withNote
                notes.reload()
            }
        }
        Item {
            Layout.fillWidth: true
        }
        Label {
            text: (notes.rows.length === 1 ? qsTr("1 session") : qsTr("%1 sessions").arg(notes.rows.length))
                + (backend.sessionsHasMore ? qsTr(" so far") : "")
            textFormat: Text.PlainText
            color: notes.win.muted
            font.family: notes.win.mono
            font.pixelSize: 12
        }
    }

    Repeater {
        model: notes.rows
        NoteRow {
            required property var modelData
            win: notes.win
            session: modelData
            onOpen: function(session) { notes.openSession(session) }
        }
    }

    Label {
        visible: notes.rows.length === 0
        Layout.fillWidth: true
        Layout.topMargin: 24
        text: backend.sessionsLoading ? qsTr("Loading…")
            : (notes.withNote ? qsTr("No sessions with notes match these filters.") : qsTr("No sessions match these filters."))
        color: notes.win.muted
        horizontalAlignment: Text.AlignHCenter
    }

    AppButton {
        objectName: "notesLoadMore"
        visible: backend.sessionsHasMore
        Layout.alignment: Qt.AlignHCenter
        win: notes.win
        enabled: !backend.sessionsLoading
        text: backend.sessionsLoading ? qsTr("Loading…") : qsTr("Load more")
        onClicked: backend.loadMoreSessions()
    }
}
