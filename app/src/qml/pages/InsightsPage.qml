import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "../statistics"
import "../insights"

ScrollView {
    id: page
    objectName: "insightsPage"
    required property var win
    readonly property alias filterBar: filters
    readonly property var report: backend.insights
    readonly property bool hasReport: Object.keys(report).length > 0
    property string tab: "patterns"
    clip: true
    contentWidth: availableWidth

    function open() {
        backend.loadReportItems()
        reload()
    }
    function reload() { backend.loadInsights(filters.query()) }
    function blockLabel(block) {
        if (!block || block === "unplanned") return qsTr("Unplanned")
        if (block === "sleep") return qsTr("Sleep")
        var index = win.blockIds.indexOf(block)
        return index < 0 ? String(block) : win.blockNames[index]
    }
    Connections {
        target: backend
        function onReportsInvalidated() { if (page.visible) page.open() }
        function onTodayDayChanged() { if (page.visible && filters.range > 0) page.reload() }
        function onSessionWriteFinished(ok, message) { if (ok && page.visible) page.reload() }
        function onOfflineChanged() { if (page.visible) page.open() }
    }

    ColumnLayout {
        width: page.availableWidth
        spacing: 14
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Repeater {
                model: [{id: "patterns", title: qsTr("Patterns")}, {id: "sessions", title: qsTr("Sessions")},
                        {id: "plan", title: qsTr("Plan adherence")}]
                ChoiceChip {
                    required property var modelData
                    objectName: "insightsTab_" + modelData.id
                    win: page.win
                    text: modelData.title
                    selected: page.tab === modelData.id
                    onClicked: page.tab = modelData.id
                }
            }
            Item { Layout.fillWidth: true }
            Label {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignRight
                elide: Text.ElideLeft
                color: page.win.muted
                font.pixelSize: 13
                textFormat: Text.PlainText
                text: [filters.filterName(), filters.range === 0
                         ? page.win.shortDate(filters.customFrom) + " – " + page.win.shortDate(filters.customTo)
                         : qsTr("Last %1 days").arg(filters.range)]
                    .concat(page.tab !== "plan" && !filters.visible ? filters.activeChips.map(chip => chip.text) : [])
                    .filter(Boolean).join("  ·  ")
            }
            ChoiceChip {
                objectName: "insightsFilters"
                win: page.win
                text: qsTr("Filters")
                selected: filters.visible
                onClicked: filters.visible = !filters.visible
            }
        }
        FilterBar {
            id: filters
            win: page.win
            Layout.fillWidth: true
            visible: false
            showMetrics: page.tab !== "plan"
            onEdited: page.reload()
        }
        Label {
            visible: page.tab === "plan"
            text: qsTr("Plan adherence uses the date and assignment filters only.")
            color: page.win.muted
            font.pixelSize: 13
        }
        Label {
            visible: !page.hasReport
            text: backend.busy ? qsTr("Loading insights…") : qsTr("Insights unavailable.")
            color: page.win.muted
        }
        Patterns {
            visible: page.tab === "patterns" && page.hasReport
            Layout.fillWidth: true
            win: page.win
            report: page.report
            blockLabel: page.blockLabel
        }
        ColumnLayout {
            visible: page.tab === "sessions" && page.hasReport
            Layout.fillWidth: true
            spacing: 10
            Repeater {
                model: [{key: "best", title: qsTr("Best sessions")}, {key: "hardest", title: qsTr("Hardest sessions")}]
                ColumnLayout {
                    id: ranking
                    required property var modelData
                    readonly property var rows: page.report[modelData.key] || []
                    Layout.fillWidth: true
                    spacing: 8
                    Label {
                        text: ranking.modelData.title + "  ·  " + (ranking.rows.length === 1 ? qsTr("1 session") : qsTr("%1 sessions").arg(ranking.rows.length))
                        textFormat: Text.PlainText
                        color: page.win.text
                        font.pixelSize: 16
                        font.weight: Font.DemiBold
                    }
                    Repeater {
                        model: ranking.rows
                        NoteRow {
                            required property var modelData
                            objectName: "insightSession_" + ranking.modelData.key + "_" + modelData.id
                            win: page.win
                            session: modelData
                            onOpen: function(row) { page.win.editReportSession(row) }
                        }
                    }
                }
            }
        }
        PlanAdherence {
            visible: page.tab === "plan" && page.hasReport
            Layout.fillWidth: true
            win: page.win
            report: page.report
            blockLabel: page.blockLabel
        }
    }
}
