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
    readonly property bool hasReview: !backend.offlineMode && summary !== null
    readonly property string currentWeek: backend.reviewWeekStart("", 0)
    // The last full week until the user picks another.
    property string week: backend.reviewWeekStart("", -1)
    readonly property bool dirty: noteField.text !== String(review.note || "")
    readonly property var goalsMet: (summary && summary.goals_met) || []
    readonly property var noted: (summary && summary.sessions_with_notes) || []
    clip: true
    contentWidth: availableWidth

    function open() {
        if (!week) week = backend.reviewWeekStart("", -1)
        backend.loadReview(week)
    }
    function shift(weeks) { setWeek(backend.reviewWeekStart(week, weeks)) }
    function setWeek(value) {
        if (!value || value > currentWeek) return
        week = value
        backend.loadReview(week)
    }
    function weekText(start) {
        if (!start) return ""
        var from = Date.fromLocaleString(Qt.locale(), start, "yyyy-MM-dd")
        var to = new Date(from.getFullYear(), from.getMonth(), from.getDate() + 6)
        return win.shortDate(from) + " – " + win.shortDate(to)
    }
    function count(n, one, many) { return n === 1 ? one : many.arg(n) }
    function perHour(value) { return value === null || value === undefined ? "—" : win.numberText(value) }

    Connections {
        target: backend
        function onReviewsInvalidated() { if (page.visible) page.open() }
        function onOfflineChanged() { if (page.visible) page.open() }
        function onReviewSaveFinished(ok, message) {
            if (!ok) return
            savedLabel.opacity = 1
            savedFade.restart()
        }
    }
    onVisibleChanged: if (visible) open()
    onReviewChanged: noteField.text = String(review.note || "")

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
                    name: modelData.name
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
                selected: page.week === backend.reviewWeekStart("", -1)
                onClicked: page.setWeek(backend.reviewWeekStart("", -1))
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
                        text: qsTr("Goals met") + "  ·  " + page.goalsMet.length
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
                        text: qsTr("No goals met this week.")
                        color: page.win.muted
                        font.pixelSize: 13
                    }
                }
            }

            SectionTitle {
                objectName: "reviewNotesTitle"
                text: qsTr("Sessions with notes") + "  ·  " + page.noted.length
            }
            Repeater {
                model: page.noted
                NoteRow {
                    required property var modelData
                    objectName: "reviewSession_" + modelData.id
                    win: page.win
                    session: modelData
                    onOpen: function(row) { page.win.editReportSession(row) }
                }
            }
            Label {
                visible: page.noted.length === 0
                text: qsTr("No sessions with notes this week.")
                color: page.win.muted
                font.pixelSize: 13
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
                    }
                    RowLayout {
                        spacing: 12
                        AppButton {
                            objectName: "reviewSave"
                            win: page.win
                            primary: true
                            text: qsTr("Save")
                            enabled: page.dirty
                            onClicked: backend.saveReview(page.week, noteField.text)
                        }
                        Label {
                            id: savedLabel
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
