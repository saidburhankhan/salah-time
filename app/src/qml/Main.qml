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

    property int page: 0
    property bool quickVisible: false
    property bool entryVisible: false
    property bool helpVisible: false
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

    property alias timelineModel: todayPage.timelineModel
    property alias dragging: todayPage.dragging
    property alias dragId: todayPage.dragId
    property alias dragPointerY: todayPage.dragPointerY
    property alias dragTitle: todayPage.dragTitle
    property alias dragDetail: todayPage.dragDetail
    property alias dragDone: todayPage.dragDone
    property alias dragPayload: todayPage.dragPayload

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
    function openEntry(item) {
        if (typeof item === "string") editItem = backend.itemForId(item)
        else if (item && item.id) editItem = backend.itemForId(item.id)
        else editItem = item || ({})
        entryVisible = true
    }
    function openEntryFromParse(title) { editItem = backend.itemFromParse(title || ""); entryVisible = true }
    function locationName() { return backend.config.locationName || backend.config.location || qsTr("Location") }
    function methodName() { return backend.config.methodName || backend.config.method || "—" }
    function capitalized(s) { s = String(s || ""); return s.charAt(0).toUpperCase() + s.slice(1) }
    function weekdayId(d) { return ["sun", "mon", "tue", "wed", "thu", "fri", "sat"][d.getDay()] }
    function shortDate(value) {
        var d = value instanceof Date ? value : Date.fromLocaleString(Qt.locale(), String(value || ""), "yyyy-MM-dd")
        return Qt.locale("en_US").toString(d, "ddd d MMM")
    }
    function countdownFromNext() {
        if (!backend.nowState.nextAt) return Number(backend.nowState.nextRemainingSeconds || 0)
        var at = Date.parse(String(backend.nowState.nextAt || ""))
        if (!isNaN(at)) return Math.max(0, Math.floor((at - Date.now()) / 1000))
        return Number(backend.nowState.nextRemainingSeconds || 0)
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
        if (entryVisible && entryPopup.confirming) { entryPopup.confirming = false; return true }
        if (helpVisible) { helpVisible = false; return true }
        if (entryVisible) { entryVisible = false; return true }
        if (quickVisible) { quickVisible = false; return true }
        if (dragging) { finishDrag(true); return true }
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
    function stepStats(amount) {
        var b = statsBounds(statsMode, statsAnchor, customFrom, customTo)
        if (statsMode === "custom") {
            var n = Math.round((b.last - b.first) / 86400000) + 1
            customFrom = new Date(b.first.getFullYear(), b.first.getMonth(), b.first.getDate() + amount * n)
            customTo = new Date(b.last.getFullYear(), b.last.getMonth(), b.last.getDate() + amount * n)
            statsAnchor = customFrom
        } else if (statsMode === "month") statsAnchor = new Date(statsAnchor.getFullYear(), statsAnchor.getMonth() + amount, 1)
        else statsAnchor = new Date(statsAnchor.getFullYear(), statsAnchor.getMonth(), statsAnchor.getDate() + amount * 7)
        loadStatistics()
    }
    function loadStatistics() { var b = statsBounds(statsMode, statsAnchor, customFrom, customTo); backend.loadStatsRange(b.from, b.to) }
    function statsRangeLabel() {
        var b = statsBounds(statsMode, statsAnchor, customFrom, customTo), l = Qt.locale("en_US")
        if (statsMode === "month") return l.toString(b.first, "MMMM yyyy")
        if (b.first.getMonth() === b.last.getMonth()) return l.toString(b.first, "d") + "–" + l.toString(b.last, "d MMM yyyy")
        return l.toString(b.first, "d MMM") + " – " + l.toString(b.last, "d MMM yyyy")
    }
    function formatMinutes(n) { n = Number(n || 0); return n >= 60 ? Math.floor(n / 60) + " h " + (n % 60) + " m" : n + " m" }
    function isTyping() { var f = win.activeFocusItem; return f && f.inputMethodHints !== undefined }

    GlobalShortcuts {
        win: win
    }

    onActiveChanged: if (active) backend.reload()

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
            win: win
        }
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            ColumnLayout {
                anchors.fill: parent
                spacing: 0
                AppHeader {
                    win: win
                }
                ErrorBanner {
                    win: win
                }

                TodayPage {
                    id: todayPage
                    visible: win.page === 0
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 28
                    win: win
                }
                RoutinesPage {
                    visible: win.page === 1
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 30
                    win: win
                }
                StatisticsPage {
                    visible: win.page === 2
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 26
                    win: win
                    tip: tip
                }
                SettingsPage {
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

    Rectangle {
        id: tip
        objectName: "statsTooltip"
        z: 3000
        visible: false
        property alias text: tipLabel.text
        width: tipLabel.implicitWidth + 24
        height: 38
        radius: 7
        color: "#35322c"
        border.color: "#575249"
        Label {
            id: tipLabel
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
    ShortcutHelp {
        win: win
    }
    Toast {
        id: toast
        win: win
    }
}
