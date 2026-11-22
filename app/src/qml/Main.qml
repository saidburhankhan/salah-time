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
    property int countdown: backend.dayPlan.now && backend.dayPlan.now.next ? backend.dayPlan.now.next.remaining_seconds : 0
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
    property alias dragPayload: todayPage.dragPayload
    property alias dragPointerY: todayPage.dragPointerY

    function beginDrag(rowIndex, id, globalY) { todayPage.beginDrag(rowIndex, id, globalY) }
    function updateDrag(globalY) { todayPage.updateDrag(globalY) }
    function finishDrag(cancelled) { todayPage.finishDrag(cancelled) }
    function syncTimeline() { todayPage.syncTimeline() }

    function openQuick(block) {
        quickBlock = block || ""
        backend.parseQuick("")
        quickVisible = true
        quickPopup.reset()
    }
    function openEntry(item) { editItem = item || ({}); entryVisible = true }
    function blockItems(id) {
        var out = [], blocks = backend.dayPlan.blocks || []
        for (var i = 0; i < blocks.length; i++)
            if (blocks[i].start === id) return blocks[i].items || []
        return out
    }
    function locationName() {
        var c = backend.config || {}
        return c.location_name || c.name
            || (typeof c.location === "string" ? c.location : (c.location ? c.location.name : ""))
            || qsTr("Location")
    }
    function methodName() { var c = backend.config || {}; return c.method_name || c.method || "—" }
    function capitalized(s) { s = String(s || ""); return s.charAt(0).toUpperCase() + s.slice(1) }
    function weekdayText(days, repeat) {
        if (repeat === "daily") return qsTr("Daily")
        var a = days || [], key = a.join(",")
        if (key === "mon,tue,wed,thu,fri") return qsTr("Mon–Fri")
        if (key === "mon,tue,wed,thu,fri,sat,sun") return qsTr("Daily")
        var names = { mon: qsTr("Mon"), tue: qsTr("Tue"), wed: qsTr("Wed"), thu: qsTr("Thu"), fri: qsTr("Fri"), sat: qsTr("Sat"), sun: qsTr("Sun") }
        return a.map(function(d) { return names[d] || capitalized(d) }).join(", ")
    }
    function detail(x) {
        var value = x.measure === "minutes" ? qsTr("%1 / %2 min").arg(runningMinutes(x)).arg(x.target || 0) : ""
        var repeat = x.repeat === "weekly" ? weekdayText(x.weekdays, x.repeat) : x.repeat === "daily" ? qsTr("Daily") : qsTr("Once")
        return value ? value + "  ·  " + repeat : repeat
    }
    function runningMinutes(x) {
        var total = Number(x.minutes || 0), started = String(x.timer_started_at || "")
        if (started) { var ms = Date.parse(started); if (!isNaN(ms)) total += Math.max(0, Math.floor((timerNow - ms) / 60000)) }
        return total
    }
    function weekdayId(d) { return ["sun", "mon", "tue", "wed", "thu", "fri", "sat"][d.getDay()] }
    function nextOccurrence(item, fromIso) {
        var d = Date.fromLocaleString(Qt.locale(), fromIso, "yyyy-MM-dd"), repeat = item.repeat || "none", days = item.weekdays || []
        if (repeat === "none" || repeat === "daily") return d
        for (var i = 0; i < 14; i++) { if (days.indexOf(weekdayId(d)) >= 0) return d; d = new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1) }
        return d
    }
    function occursOn(item, dayIso) {
        if ((item.repeat || "none") === "none" || (item.repeat || "none") === "daily") return true
        var d = Date.fromLocaleString(Qt.locale(), dayIso, "yyyy-MM-dd")
        return (item.weekdays || []).indexOf(weekdayId(d)) >= 0
    }
    function shortDate(d) { return Qt.locale("en_US").toString(d, "ddd d MMM") }
    function maybeToast(item) {
        if (occursOn(item, backend.selectedDay)) return
        var start = Date.fromLocaleString(Qt.locale(), backend.selectedDay, "yyyy-MM-dd")
        start = new Date(start.getFullYear(), start.getMonth(), start.getDate() + 1)
        toast.show(String(item.title || qsTr("Item")) + qsTr(" — next %1").arg(shortDate(nextOccurrence(item, iso(start)))))
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
    function tooltipText(day) {
        var d = Date.fromLocaleString(Qt.locale(), day.day, "yyyy-MM-dd")
        return Qt.locale("en_US").toString(d, "ddd d MMM") + " · " + (day.done || 0) + " / " + (day.planned || 0)
            + " done · " + (day.minutes || 0) + " min · " + (day.count || 0) + " counts · " + (day.prayers || 0) + " prayers"
    }
    function tooltipTextAt(index) { var rows = selectedDaily(); return index >= 0 && index < rows.length ? tooltipText(rows[index]) : "" }
    function formatMinutes(n) { n = Number(n || 0); return n >= 60 ? Math.floor(n / 60) + " h " + (n % 60) + " m" : n + " m" }
    function selectedDaily() {
        if (!selectedStatsItem) return backend.stats.daily || []
        var items = backend.stats.items || []
        for (var i = 0; i < items.length; i++)
            if (items[i].id === selectedStatsItem) {
                var out = [], s = items[i].series || []
                for (var j = 0; j < s.length; j++) out.push({ day: s[j].day, planned: s[j].planned ? 1 : 0, done: s[j].done ? 1 : 0, minutes: s[j].minutes || 0, count: s[j].count || 0, prayers: 0 })
                return out
            }
        return []
    }
    function shownDaily() { var source = selectedDaily(), out = []; for (var i = 0; i < source.length; i++) if (source[i].day <= backend.todayDay) out.push(source[i]); return out }
    function newestDaily() { var source = shownDaily(), out = []; for (var i = source.length - 1; i >= 0; i--) out.push(source[i]); return out }
    function shownTotals() {
        var rows = shownDaily(), t = { done: 0, planned: 0, minutes: 0, count: 0, prayers: 0 }
        for (var i = 0; i < rows.length; i++) { t.done += Number(rows[i].done || 0); t.planned += Number(rows[i].planned || 0); t.minutes += Number(rows[i].minutes || 0); t.count += Number(rows[i].count || 0); t.prayers += Number(rows[i].prayers || 0) }
        return t
    }
    function maxDaily(field) { var a = selectedDaily(), m = 0; for (var i = 0; i < a.length; i++) m = Math.max(m, Number(a[i][field] || 0)); return Math.max(1, m) }
    function niceAxis(field) {
        var maximum = field === "done" ? Math.max(maxDaily("planned"), maxDaily("done")) : maxDaily("minutes")
        var steps = field === "done" ? [1, 2, 5, 10, 20, 50, 100] : [5, 10, 15, 30, 60, 120, 180, 240]
        var step = steps[steps.length - 1]
        for (var i = 0; i < steps.length; i++) {
            var lines = Math.ceil(maximum / steps[i])
            if (lines >= 3 && lines <= 4) { step = steps[i]; break }
            if (lines < 3) { step = steps[i]; break }
        }
        return { step: step, maximum: Math.max(step * 3, Math.ceil(maximum / step) * step), lines: Math.max(3, Math.ceil(maximum / step)) }
    }
    function axisLabel(field, value) {
        if (field !== "minutes") return String(value)
        if (value === 0) return "0"
        var h = Math.floor(value / 60), m = value % 60
        return h ? (h + "h" + (m ? String(m).padStart(2, "0") : "")) : (value + "m")
    }
    function isTyping() { var f = win.activeFocusItem; return f && f.inputMethodHints !== undefined }

    GlobalShortcuts {
        win: win
    }

    onActiveChanged: if (active) backend.reload()

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
