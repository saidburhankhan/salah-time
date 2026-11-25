import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../statistics"

// Statistics shares one filter bar across Charts, Table and Notes and
// remembers the active tab.
ScrollView {
    id: statsPage
    objectName: "statsScroll"
    property var win
    property var tip
    // "charts", "table" or "notes".
    property string tab: "charts"
    readonly property var tabs: [
        { id: "charts", name: qsTr("Charts") },
        { id: "table", name: qsTr("Table") },
        { id: "notes", name: qsTr("Notes") }
    ]
    readonly property alias filterBar: filterBar
    readonly property var projectChoices: filterBar.projectChoices
    readonly property var taskChoices: filterBar.taskChoices
    readonly property var stats: backend.statsData
    clip: true
    contentWidth: availableWidth

    Component.onCompleted: {
        var saved = String(backend.uiSetting("statistics/tab", "charts"))
        if (saved === "charts" || saved === "table" || saved === "notes")
            tab = saved
    }

    function open() {
        backend.loadReportItems()
        reload()
    }
    function reload() {
        var query = filterBar.query()
        if (tab === "table")
            generalTable.load(query)
        else if (tab === "notes")
            notesList.load(query)
        else {
            backend.loadStatsQuery(query)
            charts.trends.load(query)
        }
    }
    function clearTrendHighlight() {
        charts.trends.clearHighlight()
    }
    function setTab(id) {
        if (tab === id)
            return
        tab = id
        backend.setUiSetting("statistics/tab", id)
        reload()
    }
    // Kept for the shortcuts and tests: forwarded to the filter bar.
    function setFilter(kind, id) {
        filterBar.setFilter(kind, id)
    }
    function setRange(days) {
        filterBar.setRange(days)
    }
    function setCustom(from, to) {
        filterBar.setCustom(from, to)
    }
    function rangeLabel() {
        var range = tab === "table" ? backend.statsTable.range : (tab === "charts" ? stats.range : null)
        var name = filterBar.filterName()
        var text = ""
        if (range)
            text = win.shortDate(range.from) + " – " + win.shortDate(range.to)
        else if (filterBar.range === 0)
            text = win.shortDate(filterBar.customFrom) + " – " + win.shortDate(filterBar.customTo)
        else
            text = qsTr("last %1 days").arg(filterBar.range)
        return (name !== "" ? name + "  ·  " : "") + text
    }

    Connections {
        target: backend
        function onReportsInvalidated() {
            if (statsPage.visible) statsPage.open()
        }
        function onSessionWriteFinished(ok, message) {
            if (ok && statsPage.visible && statsPage.tab === "notes")
                notesList.reload()
        }
        function onStatsDataChanged() {
            if (statsPage.stats.range)
                filterBar.rangeTo = String(statsPage.stats.range.to || "")
        }
    }

    ColumnLayout {
        width: statsPage.availableWidth
        spacing: 16

        FilterBar {
            id: filterBar
            Layout.fillWidth: true
            win: statsPage.win
            onEdited: statsPage.reload()
        }

        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: statsPage.win.border
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Repeater {
                model: statsPage.tabs
                ChoiceChip {
                    required property var modelData
                    objectName: "statsTab_" + modelData.id
                    win: statsPage.win
                    text: modelData.name
                    selected: statsPage.tab === modelData.id
                    onClicked: statsPage.setTab(modelData.id)
                }
            }
            Item {
                Layout.fillWidth: true
            }
            Label {
                text: statsPage.rangeLabel()
                textFormat: Text.PlainText
                color: statsPage.win.muted
                font.family: statsPage.win.mono
                font.pixelSize: 12
            }
        }

        ChartsView {
            id: charts
            metrics: filterBar.metricChoices
            visible: statsPage.tab === "charts"
            Layout.fillWidth: true
            win: statsPage.win
            tip: statsPage.tip
        }

        GeneralTable {
            id: generalTable
            visible: statsPage.tab === "table"
            win: statsPage.win
        }

        NotesList {
            id: notesList
            visible: statsPage.tab === "notes"
            Layout.fillWidth: true
            win: statsPage.win
        }

        Label {
            visible: backend.busy && statsPage.tab === "charts"
            Layout.fillWidth: true
            text: qsTr("Loading statistics…")
            color: statsPage.win.muted
            horizontalAlignment: Text.AlignHCenter
        }
        Item {
            Layout.preferredHeight: 12
        }
    }
}
