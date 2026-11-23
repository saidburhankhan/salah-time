import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// One session of the Notes tab: day and time, project and task, minutes, its
// metric values (scale values tinted like the table), the main note and the
// timestamped entries. A click opens the session editor.
Rectangle {
    id: row
    property var win
    property var session: ({})
    readonly property var project: session.project || null
    readonly property var item: session.item || null
    readonly property bool tick: session.minutes === null || session.minutes === undefined
    readonly property var entries: session.notes || []
    readonly property var valueList: {
        var list = []
        var values = session.values || {}
        var order = (backend.metrics || []).map(function(m) { return String(m.id) })
        for (var id in values) {
            var metric = win.metricById(id) || (id === "distractions" ? win.distractionMetric() : null)
            var at = order.indexOf(id)
            list.push({ id: id, value: values[id], metric: metric, position: at < 0 ? order.length : at })
        }
        // Catalog order (focus, quality, …), not the order the JSON came in.
        list.sort(function(a, b) { return a.position - b.position })
        return list
    }
    signal open(var session)
    Layout.fillWidth: true
    implicitHeight: body.implicitHeight + 28
    radius: theme.radius
    color: area.containsMouse ? win.card : win.panel
    border.color: win.border

    function timeText() {
        var start = win.clockTime(session.started_at)
        var end = win.clockTime(session.ended_at)
        if (start !== "" && end !== "")
            return start + "–" + end
        return start !== "" ? start : end
    }
    function valueText(entry) {
        var name = entry.metric ? String(entry.metric.name) : entry.id
        if (entry.metric && entry.metric.kind === "check")
            return name + (Number(entry.value) ? " ✓" : " ✗")
        if (entry.metric && entry.metric.kind === "scale")
            return name + " " + win.numberText(entry.value) + "/" + (entry.metric.scale_max || 5)
        var unit = entry.metric && entry.metric.unit && entry.metric.unit !== "times" ? " " + entry.metric.unit : ""
        return name + " " + win.numberText(entry.value) + unit
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: row.open(row.session)
    }

    ColumnLayout {
        id: body
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 14
        anchors.leftMargin: 18
        anchors.rightMargin: 18
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            spacing: 12
            Label {
                Layout.preferredWidth: 96
                text: row.win.shortDate(row.session.day)
                textFormat: Text.PlainText
                color: row.win.muted
                font.family: row.win.mono
                font.pixelSize: 12
            }
            Label {
                Layout.preferredWidth: 96
                text: row.timeText()
                textFormat: Text.PlainText
                color: row.win.muted
                font.family: row.win.mono
                font.pixelSize: 12
                font.features: { "tnum": 1 }
            }
            Rectangle {
                visible: !!row.project
                width: 10
                height: 10
                radius: 5
                color: row.project ? String(row.project.color || "#78909c") : "transparent"
            }
            Label {
                visible: !!row.project
                text: row.project ? String(row.project.name || "") : ""
                textFormat: Text.PlainText
                color: row.win.muted
                font.pixelSize: 14
            }
            Label {
                Layout.fillWidth: true
                text: row.item ? String(row.item.title || "") : (row.project ? "" : qsTr("No task"))
                textFormat: Text.PlainText
                color: row.item ? row.win.text : row.win.muted
                font.pixelSize: 15
                font.weight: Font.Medium
                elide: Text.ElideRight
            }
            Label {
                text: row.tick ? qsTr("done") : row.win.durationText(row.session.minutes)
                textFormat: Text.PlainText
                color: row.tick ? row.win.muted : row.win.text
                font.family: row.win.mono
                font.pixelSize: 13
                font.features: { "tnum": 1 }
            }
        }

        Flow {
            visible: row.valueList.length > 0
            Layout.fillWidth: true
            Layout.leftMargin: 216
            spacing: 6
            Repeater {
                model: row.valueList
                Rectangle {
                    required property var modelData
                    readonly property bool scale: !!modelData.metric && modelData.metric.kind === "scale"
                    width: pill.implicitWidth + 16
                    height: 24
                    radius: 4
                    color: scale ? row.win.scaleTint(modelData.value, modelData.metric.scale_max) : "#2a2823"
                    Label {
                        id: pill
                        anchors.centerIn: parent
                        text: row.valueText(parent.modelData)
                        textFormat: Text.PlainText
                        color: row.win.text
                        font.pixelSize: 12
                        font.features: { "tnum": 1 }
                    }
                }
            }
        }

        Label {
            visible: !!row.session.note
            Layout.fillWidth: true
            Layout.leftMargin: 216
            text: String(row.session.note || "")
            textFormat: Text.PlainText
            color: row.win.text
            font.pixelSize: 14
            wrapMode: Text.Wrap
        }

        Repeater {
            model: row.entries
            Label {
                required property var modelData
                Layout.fillWidth: true
                Layout.leftMargin: 216
                text: row.win.noteEntryText(modelData)
                textFormat: Text.PlainText
                color: row.win.muted
                font.pixelSize: 13
                wrapMode: Text.Wrap
            }
        }
    }
}
