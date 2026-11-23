import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// One dialog for the session writes that carry metrics:
//   pause — stop a timer, optionally correcting time and marking its item done
//   done — the done panel: tick the item, with its metrics and a note
//          ("Done" saves what is filled, "Skip" ticks with no values)
//   log  — "+ time" on an item: minutes plus metrics
//   edit — an existing session: minutes (not for a tick), metrics, note
Popup {
    id: prompt
    objectName: "sessionPrompt"
    property var win
    property string mode: "log"
    property string sessionId: ""
    property string itemId: ""
    property string projectId: ""
    property string title: ""
    property string color: ""
    // edit of a tick session: no minutes
    property bool tick: false
    property var metrics: []
    property var values: ({})
    property var initialValues: ({})
    property var notes: []
    property int initialMinutes: 0
    // pause: the most minutes the server accepts (0 = unknown), and whether
    // the timer ran past its day's Sleep.
    property int maxMinutes: 0
    property bool stale: false
    readonly property int workedCap: maxMinutes > 0 ? maxMinutes : Math.max(1, initialMinutes)
    property bool waiting: false
    property string serverError: ""
    readonly property bool pauseMode: mode === "pause"
    readonly property bool needsMinutes: mode === "log" || (mode === "edit" && !tick)
    readonly property int minutes: Number(minutesField.text || 0)
    readonly property bool canSave: !waiting && (!needsMinutes || minutes > 0)

    visible: win ? win.sessionPromptVisible : false
    modal: true
    focus: true
    anchors.centerIn: Overlay.overlay
    width: win ? Math.min(640, win.width - 80) : 0
    height: win ? Math.min(body.implicitHeight + 56, win.height - 60) : 0
    padding: 0
    closePolicy: Popup.NoAutoClose
    Overlay.modal: Rectangle { color: "#b3000000" }
    background: Rectangle {
        color: "#1c1b18"
        radius: Math.max(8, theme.radius)
        border.width: 1
        border.color: prompt.win.border
    }
    onClosed: win.sessionPromptVisible = false

    // options: { mode, sessionId, itemId, projectId, title, color, tick,
    //            minutes, metrics, values, note, notes }
    function show(options) {
        mode = options.mode || "log"
        sessionId = options.sessionId || ""
        itemId = options.itemId || ""
        projectId = options.projectId || ""
        title = options.title || ""
        color = options.color || ""
        tick = !!options.tick
        var initial = options.values || {}
        var copy = {}
        for (var key in initial)
            copy[key] = Number(initial[key])
        values = copy
        initialValues = copy
        metrics = win.promptMetrics(projectId, copy, options.metrics, mode === "edit")
        notes = options.notes || []
        waiting = false
        serverError = ""
        maxMinutes = options.maxMinutes || 0
        stale = !!options.stale
        initialMinutes = options.minutes || 0
        if (mode === "pause" && maxMinutes > 0)
            initialMinutes = Math.max(1, Math.min(initialMinutes, maxMinutes))
        minutesField.text = initialMinutes ? String(initialMinutes) : ""
        noteField.text = options.note || ""
        win.sessionPromptVisible = true
        scroller.contentY = 0
        if (needsMinutes)
            minutesField.forceActiveFocus()
    }

    function setValue(id, value) {
        var copy = {}
        for (var key in values)
            copy[key] = values[key]
        if (value === undefined || value === null || (typeof value === "number" && isNaN(value)))
            delete copy[id]
        else
            copy[id] = value
        values = copy
    }

    function save(skip, markDone) {
        if (!canSave)
            return
        if (pauseMode && minutesField.text !== "" && !minutesField.acceptableInput) {
            serverError = qsTr("Enter 1 to %1 minutes, or leave Worked empty.").arg(workedCap)
            minutesField.forceActiveFocus()
            return
        }
        waiting = true
        serverError = ""
        var note = skip ? "" : noteField.text.trim()
        var chosen = skip ? {} : values
        if (pauseMode) {
            // Preserve any counts added from another surface while this panel is open.
            var edits = {}
            for (var key in chosen) {
                if (chosen[key] !== initialValues[key])
                    edits[key] = chosen[key]
            }
            var adjustedMinutes = minutes > 0 && minutes !== initialMinutes ? minutes : -1
            backend.stopSession(sessionId, edits, note, markDone === true && itemId !== "", adjustedMinutes)
        } else if (mode === "done")
            backend.tickItem(itemId, chosen, note)
        else if (mode === "edit")
            backend.editSession(sessionId, tick ? 0 : minutes, chosen, note)
        else
            backend.logSession(itemId, itemId === "" ? projectId : "", minutes, chosen, note)
    }

    Connections {
        target: backend
        function onSessionWriteFinished(ok, message) {
            if (!prompt.waiting)
                return
            prompt.waiting = false
            if (ok)
                prompt.win.sessionPromptVisible = false
            else
                prompt.serverError = message
        }
    }

    Flickable {
        id: scroller
        anchors.fill: parent
        anchors.margins: 28
        clip: true
        contentWidth: width
        contentHeight: body.implicitHeight
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        ColumnLayout {
            id: body
            width: parent.width
            spacing: 18

            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                Rectangle {
                    visible: prompt.color !== ""
                    width: 12
                    height: 12
                    radius: 6
                    color: prompt.color || "transparent"
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    Label {
                        text: prompt.pauseMode ? qsTr("PAUSE SESSION")
                            : prompt.mode === "done" ? qsTr("MARK DONE")
                            : prompt.mode === "edit" ? (prompt.tick ? qsTr("EDIT DONE") : qsTr("EDIT SESSION"))
                            : qsTr("ADD TIME")
                        color: prompt.win.muted
                        font.family: prompt.win.mono
                        font.pixelSize: 12
                        font.letterSpacing: 1.5
                    }
                    Label {
                        Layout.fillWidth: true
                        text: prompt.title
                        textFormat: Text.PlainText
                        color: prompt.win.text
                        font.pixelSize: 20
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }
                }
            }

            ColumnLayout {
                visible: prompt.pauseMode || prompt.needsMinutes
                Layout.fillWidth: true
                spacing: 8
                Label {
                    text: prompt.pauseMode ? qsTr("Worked") : qsTr("Minutes")
                    color: prompt.win.muted
                    font.pixelSize: 13
                }
                RowLayout {
                    spacing: 8
                    TextField {
                        id: minutesField
                        objectName: "sessionMinutesField"
                        Layout.preferredWidth: 110
                        placeholderText: prompt.pauseMode ? qsTr("min") : ""
                        font.family: prompt.win.mono
                        inputMethodHints: Qt.ImhDigitsOnly
                        validator: IntValidator {
                            bottom: 1
                            top: prompt.pauseMode ? prompt.workedCap : 1440
                        }
                        onAccepted: prompt.save(false)
                    }
                    Repeater {
                        model: prompt.pauseMode ? [5, 15, 30] : [15, 30, 45, 60, 90]
                        ChoiceChip {
                            required property int modelData
                            objectName: prompt.pauseMode ? "subtractWorked" + modelData : ""
                            win: prompt.win
                            text: prompt.pauseMode ? qsTr("−%1").arg(modelData) : qsTr("%1 min").arg(modelData)
                            selected: !prompt.pauseMode && prompt.minutes === modelData
                            onClicked: {
                                var current = minutesField.acceptableInput ? prompt.minutes : prompt.initialMinutes
                                minutesField.text = String(prompt.pauseMode
                                    ? Math.max(1, Math.min(prompt.workedCap, current - modelData)) : modelData)
                            }
                        }
                    }
                }
                Label {
                    objectName: "sessionStaleNote"
                    visible: prompt.pauseMode && prompt.stale
                    Layout.fillWidth: true
                    text: qsTr("This timer ran past Sleep, so it stops there: at most %1 min.").arg(prompt.workedCap)
                    textFormat: Text.PlainText
                    color: prompt.win.muted
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
            }

            Label {
                visible: !prompt.pauseMode || prompt.metrics.length > 0
                text: prompt.metrics.length > 0 ? qsTr("HOW DID IT GO?  ·  all optional") : qsTr("No metrics asked.")
                color: prompt.win.muted
                font.family: prompt.metrics.length > 0 ? prompt.win.mono : ""
                font.pixelSize: prompt.metrics.length > 0 ? 11 : 13
                font.letterSpacing: prompt.metrics.length > 0 ? 1.5 : 0
            }

            Repeater {
                model: prompt.metrics
                RowLayout {
                    id: metricRow
                    required property var modelData
                    readonly property string metricId: String(modelData.metric_id)
                    readonly property var current: prompt.values[metricId]
                    Layout.fillWidth: true
                    spacing: 10
                    Label {
                        Layout.preferredWidth: 150
                        text: String(metricRow.modelData.name || "")
                        textFormat: Text.PlainText
                        color: prompt.win.text
                        font.pixelSize: 15
                        elide: Text.ElideRight
                    }
                    // scale: 1..max taps; tapping the chosen one clears it
                    Flow {
                        visible: metricRow.modelData.kind === "scale"
                        Layout.fillWidth: true
                        spacing: 6
                        Repeater {
                            model: metricRow.modelData.kind === "scale"
                                ? Number(metricRow.modelData.scale_max || 5) : 0
                            ChoiceChip {
                                required property int index
                                win: prompt.win
                                implicitWidth: 46
                                leftPadding: 0
                                rightPadding: 0
                                text: String(index + 1)
                                selected: metricRow.current === index + 1
                                onClicked: prompt.setValue(metricRow.metricId,
                                    metricRow.current === index + 1 ? undefined : index + 1)
                            }
                        }
                    }
                    // count: a number with its unit
                    TextField {
                        objectName: metricRow.metricId === "distractions" ? "promptDistractions" : ""
                        visible: metricRow.modelData.kind === "count"
                        Layout.preferredWidth: 120
                        text: metricRow.current === undefined ? "" : String(metricRow.current)
                        placeholderText: metricRow.metricId === "distractions" ? "0" : ""
                        font.family: prompt.win.mono
                        inputMethodHints: Qt.ImhFormattedNumbersOnly
                        validator: DoubleValidator { bottom: 0; decimals: 2 }
                        onTextEdited: prompt.setValue(metricRow.metricId,
                            text.trim() === "" ? undefined : Number(text))
                    }
                    Label {
                        visible: metricRow.modelData.kind === "count" && !!metricRow.modelData.unit
                        text: String(metricRow.modelData.unit || "")
                        textFormat: Text.PlainText
                        color: prompt.win.muted
                    }
                    // check: yes / no
                    Row {
                        visible: metricRow.modelData.kind === "check"
                        spacing: 6
                        ChoiceChip {
                            win: prompt.win
                            text: qsTr("Yes")
                            selected: metricRow.current === 1
                            onClicked: prompt.setValue(metricRow.metricId, metricRow.current === 1 ? undefined : 1)
                        }
                        ChoiceChip {
                            win: prompt.win
                            text: qsTr("No")
                            selected: metricRow.current === 0
                            onClicked: prompt.setValue(metricRow.metricId, metricRow.current === 0 ? undefined : 0)
                        }
                    }
                    Item {
                        visible: metricRow.modelData.kind !== "scale"
                        Layout.fillWidth: true
                    }
                }
            }

            TextField {
                id: noteField
                objectName: "sessionNoteField"
                Layout.fillWidth: true
                placeholderText: qsTr("Note (optional)")
                onAccepted: prompt.save(false)
            }

            Repeater {
                model: prompt.notes
                Label {
                    required property var modelData
                    Layout.fillWidth: true
                    text: prompt.win.noteEntryText(modelData)
                    textFormat: Text.PlainText
                    color: prompt.win.muted
                    wrapMode: Text.Wrap
                }
            }

            Label {
                visible: prompt.serverError !== ""
                Layout.fillWidth: true
                text: prompt.serverError
                textFormat: Text.PlainText
                color: "#ef8068"
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 10
                Item {
                    Layout.fillWidth: true
                }
                AppButton {
                    win: prompt.win
                    text: qsTr("Cancel")
                    onClicked: prompt.win.sessionPromptVisible = false
                }
                AppButton {
                    objectName: "sessionSkipButton"
                    visible: prompt.mode === "done"
                    win: prompt.win
                    enabled: !prompt.waiting
                    text: qsTr("Skip")
                    onClicked: prompt.save(true)
                }
                AppButton {
                    objectName: "sessionSaveButton"
                    win: prompt.win
                    primary: true
                    enabled: prompt.canSave
                    text: prompt.mode === "done" ? qsTr("Done") : qsTr("Save")
                    onClicked: prompt.save(false)
                }
                AppButton {
                    objectName: "sessionSaveDoneButton"
                    visible: prompt.pauseMode && prompt.itemId !== ""
                    win: prompt.win
                    enabled: prompt.canSave
                    text: qsTr("Save & Done")
                    onClicked: prompt.save(false, true)
                }
            }
        }
    }
}
