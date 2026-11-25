import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../statistics"
import "../goals/GoalText.js" as GoalText

// Weekly review: a Monday-based week picker, the computed summary and one
// optional reflection. Everything is optional; nothing is required to save.
ScrollView {
    id: page
    objectName: "reviewPage"
    required property var win
    readonly property var review: backend.review
    readonly property var summary: review.summary || null
    readonly property bool hasReview: !backend.offlineMode && review.week === week && summary !== null
    readonly property string currentWeek: backend.reviewWeekStart(backend.todayDay, 0)
    readonly property string lastFullWeek: backend.reviewWeekStart(backend.todayDay, -1)
    property string pickedWeek: ""
    readonly property string week: pickedWeek || lastFullWeek
    property var reflections: ({})
    property bool settingText: false
    readonly property var reflection: stateFor(week)
    readonly property bool dirty: reflection.text !== reflection.saved
    readonly property var goalsMet: (summary && summary.goals_met) || []
    readonly property int goalsMetCount: summary ? (summary.goals_met_count ?? goalsMet.length) : 0
    readonly property var noted: (summary && summary.sessions_with_notes) || []
    readonly property int notedCount: summary ? (summary.sessions_with_notes_count ?? noted.length) : 0
    readonly property bool notesArePreview: notedCount > noted.length || noted.some(row => row.notes_more === true)
    readonly property string noteQuery: notesSearch.text.trim()
    property int notesRequest: 0
    property var searchRows: []
    property string notesNext: ""
    property bool searchingNotes: false
    property bool previewSearch: false
    property string notesError: ""
    readonly property alias noteFilters: noteFilters
    readonly property bool scopedNotes: !!(noteFilters.projectId || noteFilters.epicId || noteFilters.itemId)
    readonly property bool searchAcrossWeek: !!noteQuery || (scopedNotes && notesArePreview)
    readonly property var matchingNotes: {
        var rows = searchAcrossWeek && !previewSearch ? searchRows : noted
        var query = noteQuery.toLowerCase()
        return rows.filter(function(session) {
            if (noteFilters.projectId && (session.project_id || "none") !== noteFilters.projectId) return false
            if (noteFilters.epicId && (session.epic_id || "none") !== noteFilters.epicId) return false
            if (noteFilters.itemId && session.item_id !== noteFilters.itemId) return false
            return !previewSearch || !query || String(session.note || "").toLowerCase().includes(query)
                || (session.notes || []).some(entry => String(entry.text || "").toLowerCase().includes(query))
        })
    }
    clip: true
    contentWidth: availableWidth

    function stateFor(key) {
        return reflections[key] || { saved: "", text: "", pending: null }
    }
    function updateState(key, saved, text, pending) {
        var states = Object.assign({}, reflections)
        states[key] = { saved: saved, text: text, pending: pending }
        reflections = states
        if (key === week) showDraft()
    }
    function showDraft() {
        if (!noteField) return
        settingText = true
        var text = stateFor(week).text
        if (noteField.text !== text) {
            if (savedFade) savedFade.stop()
            if (savedLabel) savedLabel.opacity = 0
            noteField.text = text
        }
        settingText = false
    }
    function acceptReview() {
        if (!review.week || !review.summary) return
        var state = stateFor(review.week)
        var saved = String(review.note || "")
        var keepDraft = state.text !== state.saved || state.pending !== null
        updateState(review.week, saved, keepDraft ? state.text : saved, state.pending)
    }
    function saveDraft() {
        var state = stateFor(week)
        if (!dirty || state.pending !== null) return
        updateState(week, state.saved, state.text, state.text)
        backend.saveReview(week, state.text)
    }
    function resetSearch() {
        ++notesRequest
        searchRows = []
        notesNext = ""
        notesError = ""
        searchingNotes = searchAcrossWeek && hasReview
        if (searchingNotes) noteSearchDelay.restart()
        else noteSearchDelay.stop()
    }
    function requestNotes(after) {
        searchingNotes = true
        notesError = ""
        backend.searchReviewNotes(++notesRequest, week, noteQuery, after)
    }
    function appendNotes(rows) {
        var merged = searchRows.slice()
        for (var row of rows) {
            var index = merged.findIndex(session => session.id === row.id)
            if (index < 0) {
                merged.push(row)
                continue
            }
            // A session's matching entries can span multiple pages.
            var entries = (merged[index].notes || []).slice()
            for (var note of row.notes || []) {
                if (!entries.some(entry => entry.id === note.id))
                    entries.push(note)
            }
            entries.sort((left, right) => String(left.at).localeCompare(String(right.at)))
            merged[index] = Object.assign({}, row, {notes: entries})
        }
        searchRows = merged
    }
    function open() {
        backend.loadReportItems()
        loadWeek()
    }
    function loadWeek() { if (week) backend.loadReview(week) }
    function shift(weeks) { setWeek(backend.reviewWeekStart(week, weeks)) }
    function setWeek(value) {
        if (!value || value > currentWeek) return
        pickedWeek = value
        loadWeek()
    }
    function weekText(start) {
        if (!start) return ""
        var from = Date.fromLocaleString(Qt.locale(), start, "yyyy-MM-dd")
        var to = new Date(from.getFullYear(), from.getMonth(), from.getDate() + 6)
        return win.shortDate(from) + " – " + win.shortDate(to)
    }
    function count(n, one, many) { return n === 1 ? one : many.arg(n) }
    function perHour(value) { return value === null || value === undefined ? "—" : win.numberText(value) }

    Timer {
        id: noteSearchDelay
        interval: 220
        onTriggered: page.requestNotes("")
    }
    Connections {
        target: backend
        function onReviewNotesLoaded(request, result, error, methodUnavailable) {
            if (request !== page.notesRequest) return
            page.searchingNotes = false
            page.previewSearch = methodUnavailable
            if (methodUnavailable) return
            page.notesError = error
            if (error) return
            page.appendNotes(result.rows || [])
            page.notesNext = String(result.next || "")
        }
        function onReviewsInvalidated() { if (page.visible) page.open() }
        function onOfflineChanged() { if (page.visible) page.open() }
        function onTodayDayChanged() { if (page.visible) Qt.callLater(page.loadWeek) }
        function onReviewSaveFinished(ok, message, key, submitted) {
            var state = page.stateFor(key)
            if (state.pending !== submitted) return
            var saved = ok ? (submitted.trim() ? submitted : "") : state.saved
            var matches = state.text === submitted
            page.updateState(key, saved, ok && matches ? saved : state.text, null)
            if (ok && key === page.week && matches) savedFade.restart()
        }
    }
    onVisibleChanged: if (visible) open()
    onReviewChanged: { acceptReview(); resetSearch() }
    onNoteQueryChanged: resetSearch()
    onSearchAcrossWeekChanged: resetSearch()
    onWeekChanged: {
        showDraft()
        if (notesSearch) notesSearch.clear()
        if (savedFade) savedFade.stop()
        if (savedLabel) savedLabel.opacity = 0
    }
    Component.onCompleted: { acceptReview(); showDraft() }

    component SectionTitle: Label {
        color: page.win.text
        font.pixelSize: 16
        font.weight: Font.DemiBold
        textFormat: Text.PlainText
    }
    component Total: RowLayout {
        id: total
        property string name: ""
        property real minutes: 0
        property real maxMinutes: 1
        Layout.fillWidth: true
        spacing: 12
        Label {
            Layout.preferredWidth: 150
            text: total.name
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: page.win.text
            font.pixelSize: 13
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 6
            radius: 3
            color: page.win.border
            Rectangle {
                width: parent.width * total.minutes / Math.max(1, total.maxMinutes)
                height: parent.height
                radius: 3
                color: page.win.selectedBorder
            }
        }
        Label {
            Layout.preferredWidth: 64
            horizontalAlignment: Text.AlignRight
            text: page.win.durationText(total.minutes)
            textFormat: Text.PlainText
            color: page.win.text
            font.pixelSize: 13
            font.features: { "tnum": 1 }
        }
    }
    component Totals: Pane {
        id: totals
        property string title: ""
        property var rows: []
        property bool byEpic: false
        readonly property real largest: rows.reduce((m, r) => Math.max(m, r.minutes), 1)
        Layout.fillWidth: true
        Layout.alignment: Qt.AlignTop
        padding: 18
        background: Card { win: page.win }
        contentItem: ColumnLayout {
            spacing: 9
            SectionTitle { text: totals.title }
            Repeater {
                model: totals.rows
                Total {
                    required property var modelData
                    name: {
                        var epic = totals.byEpic ? page.win.epicById(modelData.id) : null
                        var project = page.win.projectById(totals.byEpic ? epic?.project_id : modelData.id)
                        return page.win.reportLabel(modelData.name, epic?.paused || project?.archived)
                    }
                    minutes: modelData.minutes
                    maxMinutes: totals.largest
                }
            }
            Label {
                visible: totals.rows.length === 0
                text: qsTr("No tracked minutes this week.")
                color: page.win.muted
                font.pixelSize: 13
            }
        }
    }

    ColumnLayout {
        width: page.availableWidth
        spacing: 14

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            AppButton {
                objectName: "reviewPrevWeek"
                win: page.win
                text: "‹"
                Accessible.name: qsTr("Previous week")
                implicitWidth: 42
                onClicked: page.shift(-1)
            }
            Label {
                objectName: "reviewWeekLabel"
                text: page.weekText(page.week)
                textFormat: Text.PlainText
                color: page.win.text
                font.pixelSize: 16
                font.weight: Font.DemiBold
                horizontalAlignment: Text.AlignHCenter
                Layout.preferredWidth: 220
            }
            AppButton {
                objectName: "reviewNextWeek"
                win: page.win
                text: "›"
                Accessible.name: qsTr("Next week")
                implicitWidth: 42
                enabled: page.week < page.currentWeek
                onClicked: page.shift(1)
            }
            ChoiceChip {
                objectName: "reviewLastWeek"
                win: page.win
                text: qsTr("Last week")
                selected: page.week === page.lastFullWeek
                onClicked: page.setWeek(page.lastFullWeek)
            }
            ChoiceChip {
                objectName: "reviewThisWeek"
                win: page.win
                text: qsTr("This week")
                selected: page.week === page.currentWeek
                onClicked: page.setWeek(page.currentWeek)
            }
            Item { Layout.fillWidth: true }
        }

        Label {
            visible: !page.hasReview
            text: backend.offlineMode ? qsTr("Connect to the daemon to view the review.")
                : backend.busy ? qsTr("Loading review…") : qsTr("Review unavailable.")
            color: page.win.muted
        }

        ColumnLayout {
            visible: page.hasReview
            Layout.fillWidth: true
            spacing: 14

            GridLayout {
                Layout.fillWidth: true
                columns: page.availableWidth > 900 ? 2 : 1
                columnSpacing: 14
                rowSpacing: 14
                Totals {
                    objectName: "reviewByProject"
                    title: qsTr("Minutes by project")
                    rows: (page.summary && page.summary.by_project) || []
                }
                Totals {
                    objectName: "reviewByEpic"
                    title: qsTr("Minutes by epic")
                    byEpic: true
                    rows: (page.summary && page.summary.by_epic) || []
                }
            }

            Pane {
                Layout.fillWidth: true
                padding: 18
                background: Card { win: page.win }
                contentItem: ColumnLayout {
                    spacing: 10
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 24
                        ColumnLayout {
                            spacing: 2
                            Label { text: qsTr("Open backlog"); color: page.win.muted; font.pixelSize: 12 }
                            Label {
                                objectName: "reviewBacklogCount"
                                text: page.summary ? String(page.summary.open_backlog_count) : "—"
                                textFormat: Text.PlainText
                                color: page.win.text
                                font.pixelSize: 20
                            }
                        }
                        ColumnLayout {
                            spacing: 2
                            Label { text: qsTr("Distractions per hour"); color: page.win.muted; font.pixelSize: 12 }
                            Label {
                                objectName: "reviewDistractions"
                                text: page.summary ? page.perHour(page.summary.distractions_per_hour) : "—"
                                textFormat: Text.PlainText
                                color: page.win.text
                                font.pixelSize: 20
                            }
                        }
                        ColumnLayout {
                            spacing: 2
                            Label { text: qsTr("Previous week"); color: page.win.muted; font.pixelSize: 12 }
                            Label {
                                objectName: "reviewPreviousDistractions"
                                text: page.summary ? page.perHour(page.summary.previous_distractions_per_hour) : "—"
                                textFormat: Text.PlainText
                                color: page.win.muted
                                font.pixelSize: 20
                            }
                        }
                        Item { Layout.fillWidth: true }
                    }
                }
            }

            Pane {
                Layout.fillWidth: true
                padding: 18
                background: Card { win: page.win }
                contentItem: ColumnLayout {
                    spacing: 8
                    SectionTitle {
                        objectName: "reviewGoalsTitle"
                        text: qsTr("Goals met") + "  ·  " + page.goalsMetCount
                            + (page.goalsMet.length < page.goalsMetCount
                                ? "  ·  " + qsTr("%1 shown").arg(page.goalsMet.length) : "")
                    }
                    Repeater {
                        model: page.goalsMet
                        RowLayout {
                            id: met
                            required property var modelData
                            Layout.fillWidth: true
                            spacing: 12
                            Label {
                                Layout.fillWidth: true
                                text: (met.modelData.scope_label || qsTr("All work")) + "  ·  " + GoalText.metricName(page.win, met.modelData)
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: page.win.text
                                font.pixelSize: 13
                            }
                            Label {
                                text: met.modelData.period === "week" ? qsTr("Week") : page.win.shortDate(met.modelData.progress.from)
                                textFormat: Text.PlainText
                                color: page.win.muted
                                font.pixelSize: 13
                            }
                            Label {
                                Layout.preferredWidth: 150
                                horizontalAlignment: Text.AlignRight
                                text: GoalText.amount(page.win, met.modelData, met.modelData.progress.value)
                                    + " / " + GoalText.amount(page.win, met.modelData, met.modelData.target)
                                textFormat: Text.PlainText
                                color: page.win.text
                                font.pixelSize: 13
                                font.features: { "tnum": 1 }
                            }
                        }
                    }
                    Label {
                        visible: page.goalsMet.length === 0
                        text: page.goalsMetCount ? qsTr("No goal details available.") : qsTr("No goals met this week.")
                        color: page.win.muted
                        font.pixelSize: 13
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                SectionTitle {
                    objectName: "reviewNotesTitle"
                    Layout.fillWidth: true
                    text: qsTr("Sessions with notes") + "  ·  " + (page.scopedNotes || page.noteQuery
                        ? (page.previewSearch ? qsTr("%1 of %2 shown").arg(page.matchingNotes.length).arg(page.noted.length)
                            : qsTr("%1 matching shown").arg(page.matchingNotes.length))
                        : page.notedCount > page.noted.length
                            ? qsTr("%1 of %2").arg(page.noted.length).arg(page.notedCount) : page.notedCount)
                }
                ChoiceChip {
                    objectName: "reviewNotesFilterToggle"
                    win: page.win
                    text: qsTr("Filter notes")
                    selected: noteFilters.visible || page.scopedNotes
                    onClicked: noteFilters.visible = !noteFilters.visible
                }
            }
            FilterBar {
                id: noteFilters
                objectName: "reviewNotesFilters"
                Layout.fillWidth: true
                win: page.win
                visible: false
                showRange: false
                showTags: false
                showMetrics: false
            }
            TextField {
                id: notesSearch
                objectName: "reviewNotesSearch"
                Layout.fillWidth: true
                visible: page.notedCount > 0
                placeholderText: page.previewSearch && page.notesArePreview ? qsTr("Search notes shown here") : qsTr("Search notes")
                Accessible.name: placeholderText
            }
            Repeater {
                model: page.matchingNotes
                NoteRow {
                    required property var modelData
                    objectName: "reviewSession_" + modelData.id
                    win: page.win
                    session: modelData
                    onOpen: function(row) { page.win.editReportSession(Object.assign({}, row, {notes_more: row.notes_more || (!!page.noteQuery && !page.previewSearch)})) }
                }
            }
            Label {
                visible: page.searchingNotes || !!page.notesError || page.matchingNotes.length === 0
                Layout.fillWidth: true
                text: page.searchingNotes ? qsTr("Searching notes…") : page.notesError || ((page.noteQuery || page.scopedNotes)
                    ? (page.notesNext ? qsTr("No matches on the pages loaded so far.") : qsTr("No matching notes.")) : page.notedCount > 0
                        ? qsTr("No session previews available.") : qsTr("No sessions with notes this week."))
                textFormat: Text.PlainText
                wrapMode: Text.WordWrap
                color: page.win.muted
                font.pixelSize: 13
            }
            AppButton {
                objectName: "reviewMoreNotes"
                win: page.win
                visible: !!page.notesNext || !!page.notesError
                enabled: !page.searchingNotes
                text: page.notesError ? qsTr("Retry search") : qsTr("Load more matches")
                onClicked: page.requestNotes(page.notesNext)
            }

            Pane {
                Layout.fillWidth: true
                padding: 18
                background: Card { win: page.win }
                contentItem: ColumnLayout {
                    spacing: 10
                    SectionTitle { text: qsTr("Reflection") }
                    Label {
                        text: qsTr("Optional: what worked and what to change next week. Clear the text and save to remove it.")
                        color: page.win.muted
                        font.pixelSize: 13
                    }
                    TextArea {
                        id: noteField
                        objectName: "reviewNote"
                        Layout.fillWidth: true
                        Layout.preferredHeight: 120
                        wrapMode: TextEdit.Wrap
                        textFormat: TextEdit.PlainText
                        onTextChanged: {
                            if (page.settingText || !page.week) return
                            savedFade.stop()
                            savedLabel.opacity = 0
                            var state = page.stateFor(page.week)
                            page.updateState(page.week, state.saved, text, state.pending)
                        }
                    }
                    RowLayout {
                        spacing: 12
                        AppButton {
                            objectName: "reviewSave"
                            win: page.win
                            primary: true
                            text: qsTr("Save")
                            enabled: page.dirty && page.reflection.pending === null
                            onClicked: page.saveDraft()
                        }
                        Label {
                            id: savedLabel
                            objectName: "reviewSaved"
                            text: qsTr("Saved")
                            opacity: 0
                            color: page.win.muted
                            font.pixelSize: 13
                            NumberAnimation on opacity {
                                id: savedFade
                                running: false
                                from: 1
                                to: 0
                                duration: 2500
                            }
                        }
                    }
                }
            }
            Item { Layout.preferredHeight: 12 }
        }
    }
}
