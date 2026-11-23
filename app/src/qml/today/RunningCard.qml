import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// The running session, wherever it was started (here, the bar widget, the CLI).
Card {
    id: runningCard
    objectName: "runningCard"
    property var win
    property bool addingNote: false
    property bool savingNote: false
    readonly property var session: backend.running
    readonly property string sessionId: String(session.id || "")
    readonly property bool active: !!(session && session.id)
    readonly property bool stale: session.stale === true
    readonly property int maxMinutes: Number(session.max_minutes || 0)
    readonly property real distractions: Number((session.values || {}).distractions || 0)
    readonly property var notes: session.notes || []
    implicitHeight: body.implicitHeight + 36
    visible: active
    border.color: theme.accent

    onSessionIdChanged: {
        addingNote = false
        savingNote = false
        noteField.clear()
    }

    function saveNote() {
        var text = noteField.text.trim()
        if (text === "" || savingNote || backend.offlineMode)
            return
        savingNote = true
        backend.addRunningNote(sessionId, text)
    }

    Connections {
        target: backend
        function onRunningNoteFinished(id, ok) {
            if (id !== runningCard.sessionId)
                return
            runningCard.savingNote = false
            if (ok) {
                noteField.clear()
                runningCard.addingNote = false
            }
        }
    }

    ColumnLayout {
        id: body
        anchors.fill: parent
        anchors.margins: 18
        spacing: 10

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Rectangle {
                width: 8
                height: 8
                radius: 4
                color: "#ef8068"
                SequentialAnimation on opacity {
                    running: runningCard.active
                    loops: Animation.Infinite
                    NumberAnimation { to: 0.25; duration: 900 }
                    NumberAnimation { to: 1; duration: 900 }
                }
            }
            Label {
                text: qsTr("RUNNING")
                color: runningCard.win.muted
                font.family: runningCard.win.mono
                font.pixelSize: 12
                font.letterSpacing: 1.5
            }
            Item {
                Layout.fillWidth: true
            }
            Label {
                text: qsTr("since %1").arg(runningCard.win.clockTime(runningCard.session ? runningCard.session.started_at : ""))
                textFormat: Text.PlainText
                color: runningCard.win.muted
                font.family: runningCard.win.mono
                font.pixelSize: 12
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            Rectangle {
                visible: backend.runningColor !== ""
                width: 12
                height: 12
                radius: 6
                color: backend.runningColor || "transparent"
            }
            Label {
                Layout.fillWidth: true
                text: backend.runningTitle
                textFormat: Text.PlainText
                color: runningCard.win.text
                font.pixelSize: 17
                font.weight: Font.Medium
                elide: Text.ElideRight
            }
        }
        RowLayout {
            Layout.fillWidth: true
            Label {
                objectName: "runningElapsed"
                Layout.fillWidth: true
                text: runningCard.stale
                    ? (runningCard.maxMinutes > 0 ? runningCard.win.formatElapsed(runningCard.maxMinutes * 60) : "—")
                    : runningCard.win.elapsedClock(runningCard.session.started_at)
                textFormat: Text.PlainText
                font.features: { "tnum": 1 }
                color: runningCard.win.text
                font.family: runningCard.win.mono
                font.pixelSize: 30
            }
            AppButton {
                objectName: "runningStopButton"
                win: runningCard.win
                primary: true
                text: qsTr("❚❚  Pause")
                onClicked: runningCard.win.pauseRunning()
            }
        }
        Label {
            objectName: "runningStaleNote"
            Layout.fillWidth: true
            visible: runningCard.stale
            text: runningCard.maxMinutes > 0
                ? qsTr("Ran past Sleep: stop it to save up to %1 min").arg(runningCard.maxMinutes)
                : qsTr("Ran past Sleep: stop it to save the session")
            textFormat: Text.PlainText
            color: runningCard.win.muted
            font.pixelSize: 12
            fontSizeMode: Text.HorizontalFit
            minimumPixelSize: 10
            elide: Text.ElideRight
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 6
            Label {
                objectName: "runningDistractions"
                Layout.fillWidth: true
                text: qsTr("Distractions %1").arg(runningCard.distractions)
                textFormat: Text.PlainText
                color: runningCard.win.text
            }
            AppButton {
                objectName: "distractionMinus"
                win: runningCard.win
                leftPadding: 11
                rightPadding: 11
                text: "−"
                Accessible.name: qsTr("Subtract a distraction")
                enabled: !backend.offlineMode && runningCard.distractions > 0
                onClicked: backend.bumpDistractions(runningCard.sessionId, -1)
            }
            AppButton {
                objectName: "distractionPlus"
                win: runningCard.win
                leftPadding: 11
                rightPadding: 11
                text: "+"
                Accessible.name: qsTr("Add a distraction")
                enabled: !backend.offlineMode
                onClicked: backend.bumpDistractions(runningCard.sessionId, 1)
            }
        }

        ListView {
            id: notesList
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentHeight, 76)
            visible: count > 0
            model: runningCard.notes
            clip: true
            spacing: 4
            boundsBehavior: Flickable.StopAtBounds
            onCountChanged: Qt.callLater(positionViewAtEnd)
            ScrollBar.vertical: ScrollBar {}
            delegate: Label {
                required property var modelData
                width: notesList.width - 12
                text: runningCard.win.noteEntryText(modelData)
                textFormat: Text.PlainText
                color: runningCard.win.muted
                font.pixelSize: 12
                wrapMode: Text.Wrap
            }
        }

        AppButton {
            objectName: "runningAddNoteButton"
            visible: !runningCard.addingNote
            enabled: !backend.offlineMode
            win: runningCard.win
            text: qsTr("Add note")
            onClicked: {
                runningCard.addingNote = true
                noteField.forceActiveFocus()
            }
        }

        RowLayout {
            visible: runningCard.addingNote
            Layout.fillWidth: true
            TextField {
                id: noteField
                objectName: "runningNoteField"
                Layout.fillWidth: true
                placeholderText: qsTr("Note")
                maximumLength: 500
                enabled: !runningCard.savingNote
                onAccepted: runningCard.saveNote()
                Keys.onEscapePressed: {
                    clear()
                    runningCard.addingNote = false
                }
            }
            AppButton {
                win: runningCard.win
                text: qsTr("Save")
                enabled: !backend.offlineMode && !runningCard.savingNote && noteField.text.trim() !== ""
                onClicked: runningCard.saveNote()
            }
        }
    }
}
