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
    property string itemTitle: ""
    property string projectColor: ""
    property bool confirmingRemove: false
    readonly property bool running: !!session.running
    readonly property bool tick: !running && (session.minutes === null || session.minutes === undefined)
    readonly property string valuesText: win.valuesText(session.values)
    height: 46
    radius: theme.radius
    color: "#181714"
    border.color: running ? theme.accent : "transparent"

    RowLayout {
        anchors.fill: parent
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
                ? sessionRow.win.durationText(sessionRow.win.elapsedMinutes(sessionRow.session.started_at))
                : (sessionRow.tick ? "✓" : qsTr("%1 min").arg(sessionRow.session.minutes))
            color: sessionRow.win.text
            font.family: sessionRow.win.mono
            font.pixelSize: 13
        }
        Label {
            Layout.fillWidth: true
            text: [sessionRow.valuesText, sessionRow.session.note ? "“" + sessionRow.session.note + "”" : ""]
                .filter(function(part) { return part !== "" }).join("  ·  ")
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
            onClicked: sessionRow.win.stopRunning()
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
                values: sessionRow.session.values || {},
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
}
