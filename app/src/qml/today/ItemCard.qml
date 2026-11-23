import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// A plan line: drag handle, done box, title with project and progress, and
// the session actions. Expanded, it lists today's sessions for editing.
Rectangle {
    id: itemCard
    property var win
    property string itemId: ""
    property string title: ""
    property string state: ""
    property string measure: "check"
    property string repeat: "none"
    property string detailText: ""
    property bool pending: false
    property bool timerRunning: false
    property string timerStartedAt: ""
    property int timerMinutes: 0
    property bool timerProjected: false
    property int count: 0
    property int target: 0
    property string projectColor: ""
    property string projectId: ""
    property string projectName: ""
    property int plannedMinutes: 0
    property int actualMinutes: 0
    property int sessions: 0
    property var sessionList: []
    property var promptMetrics
    property bool ticked: false
    property string runningSessionId: ""
    property bool expanded: false
    property int rowIndex: 0
    property var list: null
    // Done is an explicit tick (day.get `done`); timed sessions never count.
    readonly property bool done: state === "done" || ticked
    readonly property bool otherRunning: !!(backend.running && backend.running.id) && runningSessionId === ""
    readonly property real headerHeight: 82
    signal toggleExpanded()
    radius: theme.radius
    color: win.dragging && win.dragId === itemId ? "transparent" : "#1c1b18"
    border.color: win.dragging && win.dragId === itemId ? "#4a463e"
        : (timerRunning ? theme.accent : win.border)
    opacity: pending ? 0.55 : 1

    // Done opens the done panel (metrics and a note, all optional); done
    // again removes the tick.
    function tapDone() {
        if (done)
            backend.untickItem(itemId)
        else
            win.openSessionPrompt({ mode: "done", itemId: itemId, projectId: projectId,
                                    title: title, color: projectColor, metrics: promptMetrics })
    }

    function addTime() {
        win.openSessionPrompt({ mode: "log", itemId: itemId, projectId: projectId,
                                title: title, color: projectColor, metrics: promptMetrics })
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        onDoubleClicked: itemCard.win.openEntry(itemCard.itemId)
    }

    RowLayout {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: itemCard.headerHeight
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        spacing: 12
        opacity: itemCard.win.dragging && itemCard.win.dragId === itemCard.itemId ? 0 : 1

        Label {
            id: handle
            text: "⠿"
            color: itemCard.win.muted
            font.pixelSize: 19
            MouseArea {
                anchors.fill: parent
                anchors.margins: -8
                preventStealing: true
                onPressed: function(mouse) {
                    var p = mapToItem(itemCard.win.contentItem, mouse.x, mouse.y)
                    itemCard.win.beginDrag(itemCard.rowIndex, itemCard.itemId, p.y,
                        itemCard.title, itemCard.detailText, itemCard.state === "done")
                }
                onPositionChanged: function(mouse) {
                    if (pressed) {
                        var p = mapToItem(itemCard.win.contentItem, mouse.x, mouse.y)
                        itemCard.win.updateDrag(p.y)
                    }
                }
                onReleased: function(mouse) {
                    var p = mapToItem(itemCard.list, mouse.x, mouse.y)
                    itemCard.win.finishDrag(p.y < 0 || p.y > itemCard.list.height)
                }
                onCanceled: itemCard.win.finishDrag(true)
            }
        }
        Rectangle {
            Layout.preferredWidth: 4
            Layout.preferredHeight: 34
            radius: 2
            color: itemCard.projectColor !== "" ? itemCard.projectColor : "#3a3730"
        }
        Rectangle {
            objectName: "doneBox"
            width: 26
            height: 26
            radius: Math.min(6, theme.radius)
            color: itemCard.done ? itemCard.win.selectedFill : "transparent"
            border.color: itemCard.done ? itemCard.win.selectedBorder : "#625e55"
            border.width: 2
            Label {
                anchors.centerIn: parent
                text: "✓"
                visible: itemCard.done
                color: itemCard.win.selectedText
                font.weight: Font.Bold
            }
            MouseArea {
                anchors.fill: parent
                anchors.margins: -6
                cursorShape: Qt.PointingHandCursor
                enabled: !itemCard.pending
                onClicked: itemCard.tapDone()
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 5
            Label {
                Layout.fillWidth: true
                text: itemCard.title
                textFormat: Text.PlainText
                color: itemCard.done ? itemCard.win.muted : itemCard.win.text
                font.pixelSize: 15
                font.weight: Font.Medium
                elide: Text.ElideRight
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 8
                Label {
                    visible: itemCard.projectName !== ""
                    text: itemCard.projectName
                    textFormat: Text.PlainText
                    color: itemCard.projectColor
                    font.pixelSize: 12
                    font.weight: Font.Medium
                }
                Label {
                    Layout.fillWidth: true
                    text: itemCard.detailText
                    textFormat: Text.PlainText
                    color: itemCard.win.muted
                    font.family: itemCard.win.mono
                    font.pixelSize: 12
                    elide: Text.ElideRight
                }
            }
        }
        // Session count; opens today's sessions.
        Rectangle {
            visible: itemCard.sessions > 0
            Layout.preferredHeight: 30
            Layout.preferredWidth: sessionsLabel.implicitWidth + 30
            radius: 15
            color: sessionsArea.containsMouse || itemCard.expanded ? "#2c2a25" : "transparent"
            border.color: itemCard.win.border
            Row {
                anchors.centerIn: parent
                spacing: 6
                Label {
                    id: sessionsLabel
                    text: itemCard.sessions === 1 ? qsTr("1 session") : qsTr("%1 sessions").arg(itemCard.sessions)
                    color: itemCard.win.text
                    font.pixelSize: 12
                }
                Label {
                    text: itemCard.expanded ? "▴" : "▾"
                    color: itemCard.win.muted
                    font.pixelSize: 11
                }
            }
            MouseArea {
                id: sessionsArea
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: itemCard.toggleExpanded()
            }
        }
        AppButton {
            objectName: "toBacklogButton"
            visible: itemCard.repeat === "none"
            win: itemCard.win
            text: qsTr("→ Backlog")
            enabled: !itemCard.pending && !itemCard.timerRunning && !backend.offlineMode
            onClicked: backend.sendToBacklog(itemCard.itemId)
        }
        AppButton {
            objectName: "addTimeButton"
            win: itemCard.win
            text: qsTr("+ time")
            enabled: !itemCard.pending
            onClicked: itemCard.addTime()
        }
        AppButton {
            objectName: "timerButton"
            win: itemCard.win
            Layout.preferredWidth: 104
            primary: itemCard.timerRunning
            enabled: !itemCard.pending && (itemCard.timerRunning || !itemCard.otherRunning)
            text: itemCard.timerRunning
                ? "❚❚ " + itemCard.win.elapsedClock(itemCard.timerStartedAt)
                : (itemCard.actualMinutes > 0 ? qsTr("▶  Resume") : qsTr("▶  Start"))
            font.family: itemCard.win.mono
            ToolTip.visible: hovered && itemCard.otherRunning
            ToolTip.text: qsTr("Pause the running session first")
            onClicked: {
                if (itemCard.timerRunning)
                    itemCard.win.pauseRunning()
                else
                    backend.startSession(itemCard.itemId)
            }
        }
    }

    Column {
        visible: itemCard.expanded
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: header.bottom
        anchors.leftMargin: 48
        anchors.rightMargin: 12
        spacing: 6
        Repeater {
            model: itemCard.expanded ? itemCard.sessionList : []
            SessionRow {
                required property var modelData
                width: parent.width
                win: itemCard.win
                session: modelData
                promptMetrics: itemCard.promptMetrics
                itemTitle: itemCard.title
                projectColor: itemCard.projectColor
            }
        }
    }
}
