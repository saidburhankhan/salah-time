import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"

// The Table tab: stats.table grouped by week (default), day, project, epic, tag or
// task. Column chips hide columns (remembered); scale metrics are tinted by
// value, minutes carry an in-cell bar scaled to the column's largest row; a
// header click sorts (descending, ascending, back to the server order).
Rectangle {
    id: table
    objectName: "generalTable"
    property var win
    property string group: "week"
    property var lastQuery: ({})
    property var hidden: []
    property string sortKey: ""
    property bool sortDesc: true
    readonly property var report: backend.statsTable
    readonly property var columns: report.columns || []
    readonly property var shown: columns.filter(function(c) { return table.hidden.indexOf(String(c.id)) < 0 })
    readonly property bool showsSessions: shown.some(function(c) { return c.id === "sessions" })
    readonly property var rows: sorted(report.rows || [], sortKey, sortDesc)
    readonly property var totals: report.totals || {}
    readonly property real minutesMax: {
        var top = 0
        var list = report.rows || []
        for (var i = 0; i < list.length; i++)
            top = Math.max(top, Number(list[i].minutes || 0))
        return top
    }
    readonly property real labelWidth: 220
    readonly property bool showsMinutes: shown.some(function(c) { return c.id === "minutes" })
    // Minutes get a wider column for their bar: 1.8 shares of the rest.
    readonly property real cellWidth: Math.max(96, (grid.width - labelWidth) / Math.max(1, shown.length + (showsMinutes ? 0.8 : 0)))
    readonly property real minutesWidth: cellWidth * 1.8
    readonly property var groups: [
        { id: "week", name: qsTr("Week") },
        { id: "day", name: qsTr("Day") },
        { id: "project", name: qsTr("Project") },
        { id: "epic", name: qsTr("Epic") },
        { id: "tag", name: qsTr("Tag") },
        { id: "item", name: qsTr("Task") }
    ]
    Layout.fillWidth: true
    implicitHeight: body.implicitHeight + 36
    radius: theme.radius
    color: win.panel
    border.color: win.border

    Component.onCompleted: {
        var saved = String(backend.uiSetting("statistics/tableHidden", ""))
        hidden = saved === "" ? [] : saved.split(",")
    }

    function load(query) {
        lastQuery = query
        backend.loadStatsTable(query, group)
    }
    function setGroup(id) {
        if (group === id)
            return
        group = id
        sortKey = ""
        backend.loadStatsTable(lastQuery, group)
    }
    function toggleColumn(id) {
        var copy = hidden.slice()
        var at = copy.indexOf(id)
        if (at >= 0)
            copy.splice(at, 1)
        else
            copy.push(id)
        hidden = copy
        backend.setUiSetting("statistics/tableHidden", copy.join(","))
    }
    function sortBy(id) {
        if (sortKey !== id) {
            sortKey = id
            sortDesc = id !== "label"
        } else if (sortDesc === (id !== "label")) {
            sortDesc = !sortDesc
        } else {
            sortKey = ""
        }
    }
    function value(row, column) {
        if (column.id === "minutes")
            return row.minutes
        if (column.id === "sessions")
            return row.sessions
        var values = row.values || {}
        return values[column.id]
    }
    function present(v) {
        return v !== null && v !== undefined && !isNaN(Number(v))
    }
    function cellText(column, v) {
        if (!present(v))
            return "—"
        if (column.id === "minutes")
            return win.durationText(v)
        if (column.id === "sessions")
            return String(v)
        return win.numberText(v)
    }
    function sorted(list, key, desc) {
        if (key === "")
            return list
        var copy = list.slice()
        var dated = group === "week" || group === "day"
        copy.sort(function(a, b) {
            if (key === "label") {
                var order = dated ? String(a.from || a.key).localeCompare(String(b.from || b.key))
                                  : String(a.label).localeCompare(String(b.label))
                return desc ? -order : order
            }
            var column = { id: key }
            var x = table.value(a, column)
            var y = table.value(b, column)
            var hasX = table.present(x)
            var hasY = table.present(y)
            if (!hasX || !hasY)
                return hasX === hasY ? 0 : (hasX ? -1 : 1)
            return desc ? Number(y) - Number(x) : Number(x) - Number(y)
        })
        return copy
    }
    function headerText(id, name) {
        var arrow = sortKey === id ? (sortDesc ? "  ▾" : "  ▴") : ""
        return name + arrow
    }
    function sampleText(row) {
        var n = Number(row.sessions || 0)
        return n === 1 ? qsTr("1 session") : qsTr("%1 sessions").arg(n)
    }

    component Title: Label {
        color: table.win.muted
        font.family: table.win.mono
        font.pixelSize: 11
        Layout.preferredWidth: 76
        Layout.alignment: Qt.AlignTop
        Layout.topMargin: 14
    }

    // One value cell: a minutes bar or a scale tint behind right-aligned text.
    component Cell: Item {
        id: cell
        property var column: ({})
        property var cellValue
        property bool total: false
        readonly property real textRoom: 80
        width: column.id === "minutes" ? table.minutesWidth : table.cellWidth
        height: parent ? parent.height : 34
        Rectangle {
            visible: cell.column.id === "minutes" && !cell.total && Number(cell.cellValue || 0) > 0
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            height: 14
            radius: 3
            width: Math.max(2, (parent.width - cell.textRoom - 12) * Number(cell.cellValue || 0) / Math.max(1, table.minutesMax))
            color: Qt.rgba(table.win.accentColor.r, table.win.accentColor.g, table.win.accentColor.b, 0.16)
        }
        Rectangle {
            visible: cell.column.kind === "scale" && table.present(cell.cellValue)
            anchors.fill: parent
            anchors.margins: 4
            radius: 4
            color: table.win.scaleTint(cell.cellValue, cell.column.scale_max)
        }
        Label {
            anchors.fill: parent
            anchors.rightMargin: 12
            horizontalAlignment: Text.AlignRight
            verticalAlignment: Text.AlignVCenter
            text: table.cellText(cell.column, cell.cellValue)
            textFormat: Text.PlainText
            color: table.present(cell.cellValue) ? table.win.text : table.win.muted
            font.family: table.win.mono
            font.pixelSize: 12
            font.features: { "tnum": 1 }
            font.weight: cell.total ? Font.DemiBold : Font.Normal
        }
    }

    ColumnLayout {
        id: body
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 18
        spacing: 12

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Title {
                text: qsTr("GROUP")
            }
            Flow {
                Layout.fillWidth: true
                spacing: 8
                Repeater {
                    model: table.groups
                    ChoiceChip {
                        required property var modelData
                        objectName: "tableGroup_" + modelData.id
                        win: table.win
                        text: modelData.name
                        selected: table.group === modelData.id
                        onClicked: table.setGroup(modelData.id)
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Title {
                text: qsTr("COLUMNS")
            }
            Flow {
                Layout.fillWidth: true
                spacing: 8
                Repeater {
                    model: table.columns
                    ChoiceChip {
                        required property var modelData
                        objectName: "tableColumn_" + modelData.id
                        win: table.win
                        text: String(modelData.name || modelData.id)
                        selected: table.hidden.indexOf(String(modelData.id)) < 0
                        onClicked: table.toggleColumn(String(modelData.id))
                    }
                }
            }
        }

        Flickable {
            id: grid
            Layout.fillWidth: true
            Layout.topMargin: 4
            implicitHeight: sheet.height
            contentWidth: Math.max(width, sheet.width)
            contentHeight: sheet.height
            flickableDirection: Flickable.HorizontalFlick
            boundsBehavior: Flickable.StopAtBounds
            clip: true
            ScrollBar.horizontal: ScrollBar {
                policy: grid.contentWidth > grid.width ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
            }

            Column {
                id: sheet
                width: table.labelWidth + table.cellWidth * table.shown.length
                    + (table.showsMinutes ? table.minutesWidth - table.cellWidth : 0)

                Row {
                    height: 40
                    Label {
                        width: table.labelWidth
                        height: parent.height
                        leftPadding: 4
                        verticalAlignment: Text.AlignVCenter
                        text: table.headerText("label", table.groups.filter(function(g) { return g.id === table.group })[0].name)
                        textFormat: Text.PlainText
                        color: table.sortKey === "label" ? table.win.selectedText : table.win.muted
                        font.family: table.win.mono
                        font.pixelSize: 11
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: table.sortBy("label")
                        }
                    }
                    Repeater {
                        model: table.shown
                        Label {
                            required property var modelData
                            objectName: "tableHeader_" + modelData.id
                            width: modelData.id === "minutes" ? table.minutesWidth : table.cellWidth
                            height: parent.height
                            rightPadding: 12
                            leftPadding: 6
                            horizontalAlignment: Text.AlignRight
                            verticalAlignment: Text.AlignVCenter
                            text: table.headerText(String(modelData.id), String(modelData.name || modelData.id))
                            textFormat: Text.PlainText
                            color: table.sortKey === String(modelData.id) ? table.win.selectedText : table.win.muted
                            font.family: table.win.mono
                            font.pixelSize: 11
                            wrapMode: Text.WordWrap
                            maximumLineCount: 2
                            elide: Text.ElideRight
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: table.sortBy(String(parent.modelData.id))
                            }
                        }
                    }
                }
                Rectangle {
                    width: sheet.width
                    height: 1
                    color: table.win.border
                }

                Repeater {
                    model: table.rows
                    Rectangle {
                        id: line
                        required property var modelData
                        required property int index
                        readonly property bool empty: Number(modelData.sessions || 0) === 0
                        width: sheet.width
                        height: 36
                        color: index % 2 === 1 ? Qt.rgba(1, 1, 1, 0.02) : "transparent"
                        Row {
                            anchors.fill: parent
                            Item {
                                width: table.labelWidth
                                height: parent.height
                                Rectangle {
                                    id: dot
                                    // Unassigned rows ("No project", "No epic") have no colour:
                                    // a muted neutral, never white or black.
                                    visible: !!line.modelData.color || line.modelData.key === "none"
                                    x: 6
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 10
                                    height: 10
                                    radius: 5
                                    color: line.modelData.color || "#5f5b53"
                                }
                                Label {
                                    anchors.left: parent.left
                                    anchors.leftMargin: dot.visible ? 24 : 4
                                    anchors.right: sample.visible ? sample.left : parent.right
                                    anchors.rightMargin: 8
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: String(line.modelData.label || line.modelData.key || "")
                                    textFormat: Text.PlainText
                                    color: line.modelData.key === "none" || line.empty ? table.win.muted : table.win.text
                                    font.pixelSize: 13
                                    elide: Text.ElideRight
                                }
                                Label {
                                    id: sample
                                    // The sample size stays visible without the Sessions column.
                                    visible: !table.showsSessions && !line.empty
                                    anchors.right: parent.right
                                    anchors.rightMargin: 8
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: table.sampleText(line.modelData)
                                    textFormat: Text.PlainText
                                    color: table.win.muted
                                    font.family: table.win.mono
                                    font.pixelSize: 11
                                }
                                // An epic row opens its page.
                                MouseArea {
                                    objectName: "epicRowOpen"
                                    anchors.fill: parent
                                    enabled: table.group === "epic" && line.modelData.key !== "none"
                                    cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                                    onClicked: table.win.openEpic(String(line.modelData.key))
                                }
                            }
                            Repeater {
                                model: table.shown
                                Cell {
                                    required property var modelData
                                    column: modelData
                                    cellValue: table.value(line.modelData, modelData)
                                }
                            }
                        }
                    }
                }

                Rectangle {
                    width: sheet.width
                    height: 1
                    color: table.win.border
                }
                Row {
                    objectName: "tableTotals"
                    height: 38
                    Label {
                        width: table.labelWidth
                        height: parent.height
                        leftPadding: 4
                        verticalAlignment: Text.AlignVCenter
                        text: table.showsSessions ? qsTr("Total")
                            : qsTr("Total  ·  %1").arg(table.sampleText(table.totals))
                        textFormat: Text.PlainText
                        color: table.win.text
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                    }
                    Repeater {
                        model: table.shown
                        Cell {
                            required property var modelData
                            column: modelData
                            total: true
                            cellValue: table.value(table.totals, modelData)
                        }
                    }
                }
            }
        }

        Label {
            visible: (table.report.rows || []).length === 0
            Layout.fillWidth: true
            text: backend.busy ? qsTr("Loading…") : qsTr("No sessions match these filters.")
            color: table.win.muted
            horizontalAlignment: Text.AlignHCenter
        }
    }
}
