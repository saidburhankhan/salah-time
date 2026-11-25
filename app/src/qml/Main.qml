import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Effects
import QtQuick.Layouts
import QtQuick.Shapes
import "pages"
import "overlays"
import "controls"
import "today"
import "backlog"
import "resources"

ApplicationWindow {
    id: win
    width: 1440
    height: 900
    minimumWidth: 960
    minimumHeight: 640
    visible: true
    title: qsTr("Salah")
    color: "#11110f"
    Material.theme: Material.Dark
    Material.accent: theme.accent
    Material.roundedScale: theme.radius

    // 0 Today, 1 Routines, 2 Statistics, 3 Settings, 4 an Epic, 5 Backlog, 6 Goals, 7 Insights, 8 Review.
    property int page: 0
    readonly property var pageByName: ({today: 0, backlog: 5, routines: 1, statistics: 2, insights: 7,
                                        goals: 6, review: 8, settings: 3})
    // `salah-app --page <name>`; an unknown name leaves Today open.
    function showPage(name) {
        var index = pageByName[String(name)]
        if (index === undefined)
            return false
        page = index
        if (name === "statistics")
            loadStatistics()
        return true
    }
    property string epicId: ""
    // The task or epic whose resource drawer is open ("" for none).
    property string resourceTargetId: ""
    property int epicBackPage: 2
    property bool quickVisible: false
    property bool entryVisible: false
    property bool helpVisible: false
    property bool sessionPromptVisible: false
    property double timerNow: Date.now()
    property string quickBlock: ""
    property var editItem: ({})
    property string statsMode: "week"
    property date statsAnchor: new Date()
    property date customFrom: new Date()
    property date customTo: new Date()
    property bool customRangeVisible: false
    property bool statsTableVisible: false
    property string selectedStatsItem: ""
    property int countdown: countdownFromNext()
    property var pendingToast: null
    property var prayerIds: ["fajr", "dhuhr", "asr", "maghrib", "isha"]
    property var blockIds: ["night", "fajr", "dhuhr", "asr", "maghrib", "isha"]
    property var blockNames: [qsTr("Before Fajr"), qsTr("Fajr"), qsTr("Dhuhr"), qsTr("Asr"), qsTr("Maghrib"), qsTr("Isha")]
    readonly property color panel: "#1b1a17"
    readonly property color card: "#201f1b"
    readonly property color border: "#34312a"
    readonly property color muted: "#8f8a80"
    readonly property color text: "#f1efe9"
    readonly property string mono: "monospace"
    // Primary and selected controls: a muted tint of the theme accent on the
    // dark surface, never a light fill. The text mixes the accent into the
    // normal text colour so it stays readable whatever the accent is.
    readonly property color accentColor: theme.accent
    readonly property color selectedFill: Qt.rgba(accentColor.r, accentColor.g, accentColor.b, 0.16)
    readonly property color selectedFillPressed: Qt.rgba(accentColor.r, accentColor.g, accentColor.b, 0.28)
    readonly property color selectedBorder: Qt.rgba(accentColor.r, accentColor.g, accentColor.b, 0.7)
    readonly property color selectedText: Qt.tint(text, Qt.rgba(accentColor.r, accentColor.g, accentColor.b, 0.55))

    property alias timelineModel: todayPage.timelineModel
    property alias dragging: todayPage.dragging
    property alias dragId: todayPage.dragId
    property alias dragPointerY: todayPage.dragPointerY
    property alias dragTitle: todayPage.dragTitle
    property alias dragDetail: todayPage.dragDetail
    property alias dragDone: todayPage.dragDone
    property alias dragPayload: todayPage.dragPayload
    property alias backlogDrag: backlogDrag

    function beginDrag(rowIndex, id, globalY, title, detail, done) { todayPage.beginDrag(rowIndex, id, globalY, title, detail, done) }
    function updateDrag(globalY) { todayPage.updateDrag(globalY) }
    function finishDrag(cancelled) { todayPage.finishDrag(cancelled) }
    function syncTimeline() { todayPage.syncTimeline() }

    function openQuick(block) {
        quickBlock = block || ""
        backend.parseQuick("")
        quickVisible = true
        quickPopup.reset()
    }
    property int entryRequest: 0
    onPageChanged: ++entryRequest
    function openEntry(item) {
        ++entryRequest
        var id = typeof item === "string" ? item : (item && item.id)
        if (id && backend.loadHistoricalTaskForEdit(id, entryRequest)) return
        editItem = id ? backend.itemForId(id) : (item || ({}))
        entryVisible = true
    }
    function openEntryFromParse(title) {
        ++entryRequest
        editItem = backend.itemFromParse(title || "")
        entryVisible = true
    }
    Connections {
        target: backend
        function onHistoricalTaskForEdit(request, row) {
            if (request !== win.entryRequest || !row.id) return
            win.editItem = backend.itemFromCurrentView(row)
            win.entryVisible = true
        }
    }
    function openBacklogEntry() { openEntry({ backlog: true }) }
    function openRoutineEnd(item, stageDate) { routineEndDialog.show(item, stageDate) }
    function openSchedule(item) { scheduleDialog.show(item) }
    // kind is "item" or "epic". Opening it again for the same target closes it.
    function openResources(kind, id, title) {
        if (resourceDrawer.opened && resourceDrawer.targetId === id) { resourceDrawer.close(); return }
        resourceDrawer.show(kind, id, title)
    }
    function locationName() { return backend.config.locationName || backend.config.location || qsTr("Location") }
    function methodName() { return backend.config.methodName || backend.config.method || "—" }
    function capitalized(s) { s = String(s || ""); return s.charAt(0).toUpperCase() + s.slice(1) }
    function weekdayId(d) { return ["sun", "mon", "tue", "wed", "thu", "fri", "sat"][d.getDay()] }
    function shortDate(value) {
        var d = value instanceof Date ? value : Date.fromLocaleString(Qt.locale(), String(value || ""), "yyyy-MM-dd")
        return Qt.locale("en_US").toString(d, "ddd d MMM")
    }
    function countdownFromNext() {
        var at = Date.parse(String(backend.nowState.nextAt || ""))
        if (isNaN(at))
            at = backend.nowState.fetchedAt + Number(backend.nowState.nextRemainingSeconds || 0) * 1000
        return Math.max(0, Math.floor((at - Date.now()) / 1000))
    }
    function timerMinutes(snapshot, started, projected, running) {
        var total = Number(snapshot || 0)
        var use = projected ? backend.nowState.fetchedAt : 0
        if (!running || !started) return total
        var ms = Date.parse(String(started))
        if (isNaN(ms)) return total
        var since = projected ? use : ms
        if (since) total += Math.max(0, Math.floor((timerNow - since) / 60000))
        return total
    }
    function maybeToast(item) {
        pendingToast = item || ({})
        backend.previewItem(item || ({}))
    }
    function closeTopOverlay() {
        if (routineEndDialog.opened) {
            if (!routineEndDialog.waiting) routineEndDialog.close()
            return true
        }
        if (resourceDrawer.opened) { resourceDrawer.close(); return true }
        if (sessionPromptVisible) { sessionPromptVisible = false; return true }
        if (entryVisible && entryPopup.saving) return true
        if (entryVisible && entryPopup.confirming) { entryPopup.confirming = false; return true }
        if (helpVisible) { helpVisible = false; return true }
        if (entryVisible) { entryVisible = false; return true }
        if (quickVisible) { quickVisible = false; return true }
        if (dragging) { finishDrag(true); return true }
        if (backlogDrag.active) { backlogDrag.finish(true); return true }
        if (statisticsPage.visible && statisticsPage.tab === "charts")
            statisticsPage.clearTrendHighlight()
        return false
    }
    function joinedDetail(block, schedule) {
        var parts = []
        if (block) parts.push(block === "night" ? qsTr("Before Fajr") : capitalized(block))
        if (schedule) parts.push(schedule)
        return parts.join("  ·  ")
    }
    function calendarDate(offset) {
        var base = Date.fromLocaleString(Qt.locale(), backend.selectedDay || backend.todayDay, "yyyy-MM-dd")
        var first = new Date(base.getFullYear(), base.getMonth(), 1)
        var monday = (first.getDay() + 6) % 7
        return new Date(first.getFullYear(), first.getMonth(), 1 - monday + offset)
    }
    function monthStep(day, amount) {
        var d = day instanceof Date ? day : Date.fromLocaleString(Qt.locale(), String(day), "yyyy-MM-dd")
        var targetFirst = new Date(d.getFullYear(), d.getMonth() + amount, 1)
        var lastDay = new Date(targetFirst.getFullYear(), targetFirst.getMonth() + 1, 0).getDate()
        return new Date(targetFirst.getFullYear(), targetFirst.getMonth(), Math.min(d.getDate(), lastDay))
    }
    function iso(d) { return d.getFullYear() + "-" + String(d.getMonth() + 1).padStart(2, "0") + "-" + String(d.getDate()).padStart(2, "0") }
    function statsBounds(mode, anchor, from, to) {
        var a = new Date(anchor.getFullYear(), anchor.getMonth(), anchor.getDate()), first, last
        if (mode === "month") { first = new Date(a.getFullYear(), a.getMonth(), 1); last = new Date(a.getFullYear(), a.getMonth() + 1, 0) }
        else if (mode === "custom") { first = new Date(from.getFullYear(), from.getMonth(), from.getDate()); last = new Date(to.getFullYear(), to.getMonth(), to.getDate()); if (first > last) { var swap = first; first = last; last = swap } }
        else { var monday = (a.getDay() + 6) % 7; first = new Date(a.getFullYear(), a.getMonth(), a.getDate() - monday); last = new Date(first.getFullYear(), first.getMonth(), first.getDate() + 6) }
        return { from: iso(first), to: iso(last), first: first, last: last }
    }
    function stepStats(amount) { statisticsPage.filterBar.stepRange(amount) }
    // The Statistics page owns its range (7/28/90 days) and filter.
    function loadStatistics() { statisticsPage.open() }
    function statsRangeLabel() {
        var b = statsBounds(statsMode, statsAnchor, customFrom, customTo), l = Qt.locale("en_US")
        if (statsMode === "month") return l.toString(b.first, "MMMM yyyy")
        if (b.first.getMonth() === b.last.getMonth()) return l.toString(b.first, "d") + "–" + l.toString(b.last, "d MMM yyyy")
        return l.toString(b.first, "d MMM") + " – " + l.toString(b.last, "d MMM yyyy")
    }
    // Settings lists (projects, tags, metrics) as plain lookups. Reading
    // backend.* inside these keeps bindings that call them live.
    function projectById(id) {
        if (!id)
            return null
        var list = backend.projects || []
        for (var i = 0; i < list.length; i++) {
            if (String(list[i].id) === String(id))
                return list[i]
        }
        return null
    }
    function projectName(id) {
        var project = projectById(id)
        return project ? String(project.name || "") : ""
    }
    function projectColor(id) {
        var project = projectById(id)
        return project ? String(project.color || "#78909c") : "transparent"
    }
    function epicById(id) {
        if (!id)
            return null
        var list = backend.epics || []
        for (var i = 0; i < list.length; i++) {
            if (String(list[i].id) === String(id))
                return list[i]
        }
        return null
    }
    function epicName(id) {
        var epic = epicById(id)
        return epic ? String(epic.name || "") : ""
    }
    // A project's epics, active first; finished ones only when asked for.
    function projectEpics(projectId, includeFinished) {
        if (!projectId)
            return []
        var list = (backend.epics || []).filter(function(e) {
            return String(e.project_id) === String(projectId) && (includeFinished || e.status !== "finished")
        })
        return list.sort(function(a, b) {
            return (a.status === "finished" ? 1 : 0) - (b.status === "finished" ? 1 : 0)
        })
    }
    function openEpic(id) {
        if (!id)
            return
        if (page !== 4)
            epicBackPage = page
        entryVisible = false
        epicId = String(id)
        page = 4
        epicPage.open()
    }
    function activeProjects() {
        return (backend.projects || []).filter(function(p) { return !p.archived })
    }
    function tagById(id) {
        var list = backend.tags || []
        for (var i = 0; i < list.length; i++) {
            if (String(list[i].id) === String(id))
                return list[i]
        }
        return null
    }
    function metricById(id) {
        var list = backend.metrics || []
        for (var i = 0; i < list.length; i++) {
            if (String(list[i].id) === String(id))
                return list[i]
        }
        return null
    }
    function metricKindText(metric) {
        if (!metric)
            return ""
        if (metric.kind === "scale")
            return qsTr("Scale 1–%1").arg(metric.scale_max || 5)
        if (metric.kind === "check")
            return qsTr("Yes / no")
        return metric.unit ? qsTr("Count · %1").arg(metric.unit) : qsTr("Count")
    }
    function projectSummary(project) {
        var parts = []
        var tagNames = (project.tag_ids || []).map(function(id) {
            var tag = tagById(id)
            return tag ? String(tag.name) : ""
        }).filter(function(name) { return name !== "" })
        if (tagNames.length > 0)
            parts.push(tagNames.join(", "))
        var metricNames = (project.metrics || []).map(function(link) {
            var metric = link.metric || metricById(link.metric_id)
            var name = metric ? String(metric.name) : String(link.metric_id)
            return name
        })
        parts.push(metricNames.length > 0 ? metricNames.join(", ") : qsTr("no metrics"))
        return parts.join("  ·  ")
    }
    function distractionMetric() {
        return metricById("distractions") || {
            id: "distractions", name: qsTr("Distractions"), unit: qsTr("times"), kind: "count"
        }
    }
    function promptMetrics(projectId, values, asked, editing) {
        var list = []
        var project = projectById(projectId)
        var seen = new Set()
        function add(metric) {
            if (!metric)
                return
            var id = String(metric.metric_id || metric.id)
            if (seen.has(id))
                return
            seen.add(id)
            list.push({ metric_id: id, name: String(metric.name || id),
                        unit: String(metric.unit || ""), kind: String(metric.kind || "scale"),
                        scale_max: metric.scale_max || 5 })
        }
        if (asked !== null && asked !== undefined) {
            for (var i = 0; i < asked.length; i++)
                add(asked[i])
        } else if (project) {
            var links = project.metrics || []
            for (var j = 0; j < links.length; j++) {
                var metric = links[j].metric || metricById(links[j].metric_id)
                if (metric && !metric.archived)
                    add(metric)
            }
        } else if (!projectId) {
            add(metricById("focus") || { id: "focus", name: qsTr("Focus"), kind: "scale", scale_max: 5 })
            add(metricById("quality") || { id: "quality", name: qsTr("Quality"), kind: "scale", scale_max: 5 })
            add(distractionMetric())
        }
        // Retain fields for recorded values even if the task's asks have changed.
        for (var key in (values || {}))
            add(key === "distractions" ? distractionMetric() : metricById(key))
        if (editing)
            add(distractionMetric())
        return list
    }
    function elapsedMinutes(startedAt) {
        var ms = Date.parse(String(startedAt || ""))
        if (isNaN(ms))
            return 0
        return Math.max(0, Math.floor((timerNow - ms) / 60000))
    }
    // A running timer as minutes:seconds, never hours, matching the widget:
    // "09:05", "62:03", "100:05".
    function elapsedClock(startedAt) {
        var ms = Date.parse(String(startedAt || ""))
        var total = isNaN(ms) ? 0 : Math.max(0, Math.floor((timerNow - ms) / 1000))
        return formatElapsed(total)
    }
    function formatElapsed(total) {
        return String(Math.floor(total / 60)).padStart(2, "0") + ":" + String(total % 60).padStart(2, "0")
    }
    function durationText(minutes) {
        minutes = Math.max(0, Math.round(Number(minutes || 0)))
        if (minutes < 60)
            return minutes + "m"
        return Math.floor(minutes / 60) + "h " + String(minutes % 60).padStart(2, "0") + "m"
    }
    function clockTime(iso) {
        var ms = Date.parse(String(iso || ""))
        return isNaN(ms) ? "" : Qt.formatTime(new Date(ms), "HH:mm")
    }
    function noteEntryText(entry) {
        return [clockTime(entry.at), String(entry.text || "")].filter(Boolean).join("  ")
    }
    function valuesText(values) {
        var parts = []
        for (var key in (values || {})) {
            var metric = metricById(key)
            var value = Number(values[key])
            if (metric && metric.kind === "check")
                parts.push(String(metric.name) + (value ? " ✓" : " ✗"))
            else
                parts.push((metric ? String(metric.name) : key) + " " + (Math.round(value * 10) / 10)
                    + (metric && metric.unit ? " " + metric.unit : ""))
        }
        return parts.join("  ·  ")
    }
    // Muted red → amber → green for a scale value (1..max) on the dark
    // surface; transparent without a value.
    function scaleTint(value, max) {
        if (value === null || value === undefined || isNaN(Number(value)))
            return "transparent"
        var top = Math.max(2, Number(max || 5))
        var t = Math.max(0, Math.min(1, (Number(value) - 1) / (top - 1)))
        var low = t < 0.5 ? Qt.color("#b5574a") : Qt.color("#c49a45")
        var high = t < 0.5 ? Qt.color("#c49a45") : Qt.color("#6c9a64")
        var f = t < 0.5 ? t * 2 : (t - 0.5) * 2
        return Qt.rgba(low.r + (high.r - low.r) * f, low.g + (high.g - low.g) * f,
                       low.b + (high.b - low.b) * f, 0.26)
    }
    // One decimal at most, no trailing ".0"; "—" without a value.
    function numberText(value) {
        if (value === null || value === undefined || isNaN(Number(value)))
            return "—"
        return String(Math.round(Number(value) * 10) / 10)
    }
    function editReportSession(session) {
        var project = session.project || null
        var item = session.item || null
        openSessionPrompt({
            mode: "edit",
            tick: session.minutes === null || session.minutes === undefined,
            version: session.version,
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

    function openSessionPrompt(options) {
        sessionPrompt.show(options)
    }
    function runningClock(session) {
        if (!session) return "—"
        if (session.stale)
            return session.max_minutes > 0 ? formatElapsed(session.max_minutes * 60) : "—"
        return elapsedClock(session.started_at)
    }
    function pauseSession(expectedId) {
        if (!expectedId || backend.running.id !== expectedId) {
            backend.reload()
            return
        }
        pauseRunning()
    }
    function pauseRunning() {
        var running = backend.running
        if (!running || !running.id)
            return
        var started = Date.parse(String(running.started_at || ""))
        var elapsed = isNaN(started) ? 0 : Math.round((Date.now() - started) / 60000)
        // The server stops a timer at its day's Sleep and accepts at most
        // max_minutes; never offer more than that.
        var cap = Number(running.max_minutes)
        var hasCap = running.max_minutes !== undefined && running.max_minutes !== null && !isNaN(cap)
        if (hasCap)
            elapsed = Math.min(elapsed, cap)
        openSessionPrompt({
            mode: "pause",
            sessionId: String(running.id),
            itemId: String(running.item_id || ""),
            title: backend.runningTitle,
            color: backend.runningColor,
            projectId: String(running.project_id || ""),
            minutes: Math.max(1, Math.min(1440, elapsed)),
            maxMinutes: hasCap ? Math.max(1, cap) : 0,
            stale: running.stale === true,
            metrics: backend.runningPrompt,
            values: running.values || {}
        })
    }
    function openSettings(section) {
        entryVisible = false
        page = 3
        settingsPage.section = section || "general"
    }
    function formatMinutes(n) { n = Number(n || 0); return n >= 60 ? Math.floor(n / 60) + " h " + (n % 60) + " m" : n + " m" }
    function isTyping() { var f = win.activeFocusItem; return f && f.inputMethodHints !== undefined }

    GlobalShortcuts {
        win: win
    }

    onActiveChanged: if (active) backend.reload()
    Component.onCompleted: backend.loadSettings()

    Connections {
        target: backend.preview
        function onChanged() {
            if (!win.pendingToast) return
            var draft = win.pendingToast
            win.pendingToast = null
            var next = backend.preview.nextOccurrence
            if (next && next !== backend.selectedDay)
                toast.show(String(draft.title || qsTr("Item")) + qsTr(" — next %1").arg(win.shortDate(next)))
        }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0
        Sidebar {
            id: sidebar
            win: win
        }
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            ColumnLayout {
                anchors.fill: parent
                // The resource drawer sits beside the page, not over it.
                anchors.rightMargin: resourceDrawer.visible ? resourceDrawer.width : 0
                spacing: 0
                AppHeader {
                    win: win
                }
                ErrorBanner {
                    win: win
                }

                Item {
                    visible: win.page === 0 || win.page === 5
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 28
                    TodayPage {
                        id: todayPage
                        anchors.fill: parent
                        visible: win.page === 0
                        win: win
                    }
                    BacklogPage {
                        id: backlogPage
                        anchors.fill: parent
                        visible: win.page === 5 || backlogDrag.active
                        // A drag handle keeps its mouse grab across navigation.
                        opacity: win.page === 5 ? 1 : 0
                        enabled: win.page === 5 || backlogDrag.active
                        win: win
                    }
                }
                RoutinesPage {
                    visible: win.page === 1
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 30
                    win: win
                }
                StatisticsPage {
                    id: statisticsPage
                    visible: win.page === 2
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 26
                    win: win
                    tip: tip
                }
                EpicPage {
                    id: epicPage
                    visible: win.page === 4
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 26
                    win: win
                }
                Loader {
                    id: insightsLoader
                    readonly property var appWindow: win
                    active: false
                    visible: win.page === 7
                    onVisibleChanged: {
                        if (visible && item) item.open()
                        else if (visible) active = true
                    }
                    onLoaded: item.open()
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 26
                    sourceComponent: InsightsPage { win: insightsLoader.appWindow }
                }
                GoalsPage {
                    id: goalsPage
                    visible: win.page === 6
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 26
                    win: win
                    onVisibleChanged: if (visible) open()
                }
                ReviewPage {
                    id: reviewPage
                    visible: win.page === 8
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 26
                    win: win
                }
                SettingsPage {
                    id: settingsPage
                    visible: win.page === 3
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    win: win
                }
            }
        }
    }

    DragGhost {
        win: win
    }
    BacklogDrag {
        id: backlogDrag
        win: win
        backlogPage: backlogPage
        todayPage: todayPage
        todayLink: sidebar.todayLink
    }
    ScheduleDialog {
        id: scheduleDialog
        win: win
    }
    ResourceDrawer {
        id: resourceDrawer
        win: win
    }

    Rectangle {
        id: tip
        objectName: "statsTooltip"
        z: 3000
        visible: false
        property alias text: tipLabel.text
        width: Math.min(win.width - 24, tipLabel.implicitWidth + 24)
        height: tipLabel.implicitHeight + 20
        radius: 7
        color: "#35322c"
        border.color: "#575249"
        Label {
            id: tipLabel
            width: parent.width - 24
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
            anchors.centerIn: parent
            color: win.text
            font.family: win.mono
            font.pixelSize: 11
        }
    }

    QuickAdd {
        id: quickPopup
        win: win
    }
    EntryForm {
        id: entryPopup
        win: win
        item: win.editItem
    }
    RoutineEndDialog {
        id: routineEndDialog
        win: win
    }
    SessionPrompt {
        id: sessionPrompt
        win: win
    }
    ShortcutHelp {
        win: win
    }
    Toast {
        id: toast
        win: win
    }
}
