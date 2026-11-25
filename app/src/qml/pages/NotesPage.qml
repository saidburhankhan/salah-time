import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../notes"
import "../statistics"

ScrollView {
    id: page
    objectName: "notesPage"
    required property var win
    readonly property alias filterBar: filters
    property string kind: "all"
    readonly property bool editingNote: win.noteIsIn(page.contentItem)
    property var displayedRows: []
    Binding {
        page.displayedRows: backend.noteRows
        when: !page.editingNote
        restoreMode: Binding.RestoreNone
    }
    property string openingId: ""
    property string openError: ""
    property int openRequest: 0
    property bool ready: false
    readonly property string scope: [filters.range === 0
            ? win.shortDate(filters.customFrom) + " – " + win.shortDate(filters.customTo)
            : qsTr("Last %1 days").arg(filters.range),
        filters.filterName(), ...filters.activeChips.map(chip => chip.text),
        kind === "main" ? qsTr("Main notes") : kind === "entry" ? qsTr("Timed notes") : "",
        search.text.trim() ? qsTr("Search “%1”").arg(search.text.trim()) : ""].filter(Boolean).join(" · ")
    clip: true
    contentWidth: availableWidth

    Component.onCompleted: {
        try {
            var saved = JSON.parse(String(backend.uiSetting("notes/filters", "{}")))
            filters.restore(saved.filter || {})
            search.text = saved.search || ""
            kind = saved.kind === "main" || saved.kind === "entry" ? saved.kind : "all"
        } catch (error) {
            // An unreadable preference leaves the default range in place.
        }
        ready = true
        if (visible) open()
    }
    onVisibleChanged: {
        if (visible && ready) open()
        else {
            ++openRequest
            openingId = ""
            searchDelay.stop()
        }
    }

    function open() {
        backend.loadReportItems()
        if (!editingNote) reload()
    }
    function reload() {
        backend.loadNotesReport(filters.query(), search.text, kind)
    }
    function changeFilters() {
        if (!ready || editingNote) return
        searchDelay.stop()
        openingId = ""
        ++openRequest
        openError = ""
        backend.setUiSetting("notes/filters", JSON.stringify({filter: filters.query(), search: search.text, kind: kind}))
        reload()
    }
    function openSession(row) {
        openingId = row.session_id
        openError = ""
        backend.loadSessionForNote(++openRequest, row.session_id, row.session)
    }

    Connections {
        target: backend
        function onReportsInvalidated() {
            if (page.visible) {
                backend.loadReportItems()
                if (!page.editingNote) page.reload()
            }
        }
        function onTodayDayChanged() { if (page.visible && filters.range > 0 && !page.editingNote) page.reload() }
        function onSessionWriteFinished(ok, message, operation) {
            if (page.visible && ok && !page.editingNote) page.reload()
        }
        function onSessionForNoteLoaded(request, session, error) {
            if (!page.visible || request !== page.openRequest) return
            page.openingId = ""
            page.openError = error
            if (!error) page.win.editReportSession(session)
        }
    }

    Timer {
        id: searchDelay
        interval: 280
        onTriggered: page.changeFilters()
    }

    ColumnLayout {
        width: page.availableWidth
        spacing: 12

        FilterBar {
            id: filters
            win: page.win
            enabled: !page.editingNote
            Layout.fillWidth: true
            onEdited: page.changeFilters()
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: page.win.border }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            TextField {
                id: search
                objectName: "catalogueNoteSearch"
                Layout.fillWidth: true
                placeholderText: qsTr("Search notes")
                enabled: !page.editingNote
                onTextEdited: searchDelay.restart()
                onAccepted: page.changeFilters()
            }
            Repeater {
                model: [{id: "all", label: qsTr("All")}, {id: "main", label: qsTr("Main")}, {id: "entry", label: qsTr("Timed")}]
                ChoiceChip {
                    required property var modelData
                    objectName: "catalogueNoteKind_" + modelData.id
                    win: page.win
                    selected: page.kind === modelData.id
                    text: modelData.label
                    enabled: !page.editingNote
                    onClicked: {
                        page.kind = modelData.id
                        page.changeFilters()
                    }
                }
            }
        }

        Label {
            Layout.fillWidth: true
            visible: backend.notesTotal > 0
            text: backend.noteRows.length < backend.notesTotal
                ? qsTr("%1 of %2 notes").arg(backend.noteRows.length).arg(backend.notesTotal)
                : backend.notesTotal === 1 ? qsTr("1 note") : qsTr("%1 notes").arg(backend.notesTotal)
            textFormat: Text.PlainText
            color: page.win.muted
            font.family: page.win.mono
            font.pixelSize: 12
        }
        Label {
            visible: !!page.openError || !!backend.notesError
            Layout.fillWidth: true
            text: page.openError || backend.notesError
            textFormat: Text.PlainText
            color: "#ef8068"
            wrapMode: Text.Wrap
        }
        Repeater {
            model: page.displayedRows
            Pane {
                id: row
                required property var modelData
                readonly property var session: modelData.session
                readonly property string key: modelData.session_id + "/" + (modelData.note_id || "")
                readonly property var project: page.win.projectById(session.project_id)
                readonly property var epic: page.win.epicById(session.epic_id)
                objectName: "catalogueNote_" + key
                Layout.fillWidth: true
                padding: 16
                background: Card { win: page.win }
                contentItem: ColumnLayout {
                    spacing: 8
                    InlineNote {
                        win: page.win
                        session: ({id: row.modelData.session_id, version: row.session.version})
                        entry: row.modelData.note_id ? {id: row.modelData.note_id} : null
                        prefix: ""
                        text: row.modelData.text
                        pixelSize: 16
                        buttonName: "catalogueNoteEdit"
                        enabled: !backend.notesLoading && !searchDelay.running
                        onCanceled: Qt.callLater(page.reload)
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10
                        Label {
                            Layout.fillWidth: true
                            text: [page.win.clockTime(row.modelData.at),
                                   row.modelData.kind === "main" ? qsTr("Main") : qsTr("Timed"),
                                   page.win.shortDate(row.session.day), row.session.title,
                                   row.project ? page.win.reportLabel(row.project.name, row.project.archived) : "",
                                   row.epic ? page.win.reportLabel(row.epic.name, row.epic.paused) : "",
                                   row.session.minutes == null ? qsTr("Done") : page.win.durationText(row.session.minutes)]
                                   .filter(Boolean).join(" · ")
                            textFormat: Text.PlainText
                            color: page.win.muted
                            wrapMode: Text.Wrap
                            font.pixelSize: 12
                        }
                        AppButton {
                            objectName: "catalogueOpenSession"
                            win: page.win
                            enabled: !page.openingId && !page.editingNote
                            text: page.openingId === row.modelData.session_id ? qsTr("Opening…") : qsTr("Open session")
                            onClicked: page.openSession(row.modelData)
                        }
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6
                        Repeater {
                            model: Object.keys(row.session.values || {})
                            Rectangle {
                                required property string modelData
                                implicitWidth: metricLabel.implicitWidth + 12
                                implicitHeight: metricLabel.implicitHeight + 6
                                color: page.win.card
                                border.color: page.win.border
                                radius: 4
                                Label {
                                    id: metricLabel
                                    anchors.centerIn: parent
                                    text: page.win.valuesText({[parent.modelData]: row.session.values[parent.modelData]})
                                    textFormat: Text.PlainText
                                    font.family: page.win.mono
                                    font.pixelSize: 11
                                    color: page.win.muted
                                }
                            }
                        }
                    }
                }
            }
        }

        Label {
            visible: backend.noteRows.length === 0 && !backend.notesError
            Layout.fillWidth: true
            Layout.topMargin: 12
            text: backend.notesLoading ? qsTr("Loading notes…") : qsTr("No notes match %1.").arg(page.scope)
            textFormat: Text.PlainText
            color: page.win.muted
            wrapMode: Text.Wrap
        }
        AppButton {
            objectName: "catalogueMoreNotes"
            visible: backend.notesHasMore
            enabled: !backend.notesLoading && !page.editingNote
            win: page.win
            text: backend.notesLoading ? qsTr("Loading…") : qsTr("Load more")
            onClicked: backend.loadMoreNotesReport()
        }
        AppButton {
            visible: !!backend.notesError && !backend.notesHasMore
            win: page.win
            text: qsTr("Retry")
            onClicked: page.reload()
        }
        Item { implicitHeight: 12 }

    }
}
