import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

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
    readonly property bool running: !!session.running
    readonly property bool tick: !running && (session.minutes === null || session.minutes === undefined)
    readonly property string valuesText: win.valuesText(session.values)
    readonly property string epicName: win.epicName(session.epic_id)
    readonly property var notes: session.notes || []
    readonly property int noteLineHeight: 18
    height: 46 + (notes.length > 0 ? notes.length * noteLineHeight + 6 : 0)
    radius: theme.radius
    color: "#181714"
    border.color: running ? theme.accent : "transparent"

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
                ? sessionRow.win.elapsedClock(sessionRow.session.started_at)
                : (sessionRow.tick ? "✓" : qsTr("%1 min").arg(sessionRow.session.minutes))
            color: sessionRow.win.text
            font.family: sessionRow.win.mono
            font.pixelSize: 13
        }
        Label {
            Layout.fillWidth: true
            text: [sessionRow.epicName, sessionRow.valuesText, sessionRow.session.note || ""]
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
            onClicked: sessionRow.win.pauseRunning()
        }
        AppButton {
            visible: !sessionRow.running && !sessionRow.confirmingRemove
            win: sessionRow.win
            text: qsTr("Edit")
            onClicked: sessionRow.win.openSessionPrompt({
                mode: "edit",
                tick: sessionRow.tick,
                sessionId: String(sessionRow.session.id),
                projectId: String(sessionRow.session.project_id || ""),
                title: sessionRow.itemTitle,
                color: sessionRow.projectColor,
                minutes: sessionRow.session.minutes || 0,
                metrics: sessionRow.promptMetrics,
                values: sessionRow.session.values || {},
                notes: sessionRow.session.notes || [],
                note: String(sessionRow.session.note || "")
            })
        }
        AppButton {
            visible: !sessionRow.running
            win: sessionRow.win
            text: sessionRow.confirmingRemove ? qsTr("Remove") : "×"
            onClicked: {
                if (sessionRow.confirmingRemove)
                    backend.removeSession(String(sessionRow.session.id))
                else
                    sessionRow.confirmingRemove = true
            }
        }
        AppButton {
            visible: sessionRow.confirmingRemove
            win: sessionRow.win
            text: qsTr("Keep")
            onClicked: sessionRow.confirmingRemove = false
        }
    }

    Column {
        visible: sessionRow.notes.length > 0
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: 46
        anchors.leftMargin: 14
        anchors.rightMargin: 12
        Repeater {
            model: sessionRow.notes
            Row {
                id: noteLine
                required property var modelData
                width: parent.width
                height: sessionRow.noteLineHeight
                spacing: 12
                Label {
                    width: 104
                    height: parent.height
                    verticalAlignment: Text.AlignVCenter
                    text: sessionRow.win.clockTime(noteLine.modelData.at)
                    color: sessionRow.win.muted
                    font.family: sessionRow.win.mono
                    font.pixelSize: 12
                }
                Label {
                    width: parent.width - 116
                    height: parent.height
                    verticalAlignment: Text.AlignVCenter
                    text: String(noteLine.modelData.text || "")
                    textFormat: Text.PlainText
                    color: sessionRow.win.text
                    font.pixelSize: 12
                    elide: Text.ElideRight
                }
            }
        }
    }
}
