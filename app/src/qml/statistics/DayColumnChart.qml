import QtQuick
import QtQuick.Controls

// Minutes per day as columns: the planned minutes as a ghost column behind
// the minutes actually tracked. Hover shows the day with its capacity.
Rectangle {
    id: chart
    objectName: "dayChart"
    property var win
    property var tip
    property string title: ""
    property string subtitle: ""
    // stats.days rows: {day, minutes, planned_minutes, capacity_minutes, fulfilment_ratio}
    property var days: []
    readonly property real maximum: {
        var top = 30
        for (var i = 0; i < days.length; i++)
            top = Math.max(top, Number(days[i].minutes || 0), Number(days[i].planned_minutes || 0))
        // Round so each of the four grid lines lands on whole 15 or 60 minutes.
        var unit = (top > 240 ? 60 : 15) * 4
        return Math.ceil(top / unit) * unit
    }
    readonly property int axisLines: 4
    width: parent ? parent.width : 0
    height: 250
    radius: theme.radius
    color: win ? win.panel : "#1b1a17"
    border.color: win ? win.border : "#34312a"

    function axisText(minutes) {
        if (minutes < 60)
            return minutes + "m"
        var rest = minutes % 60
        return Math.floor(minutes / 60) + "h" + (rest ? String(rest).padStart(2, "0") : "")
    }
    function dayLabel(index) {
        var d = Date.fromLocaleString(Qt.locale(), String(days[index].day), "yyyy-MM-dd")
        if (days.length <= 7)
            return Qt.locale("en_US").toString(d, "ddd")
        var every = days.length <= 31 ? 4 : 14
        // The last day is labelled too, unless a regular label sits right before it.
        var last = index === days.length - 1 && index % every >= every / 2
        return (index % every === 0 || last) ? Qt.locale("en_US").toString(d, "d MMM") : ""
    }
    function tooltip(row) {
        var d = Date.fromLocaleString(Qt.locale(), String(row.day), "yyyy-MM-dd")
        var parts = [Qt.locale("en_US").toString(d, "ddd d MMM"), win.formatMinutes(row.minutes)]
        if (row.planned_minutes > 0)
            parts.push(qsTr("planned %1").arg(win.formatMinutes(row.planned_minutes)))
        parts.push(qsTr("capacity %1").arg(win.formatMinutes(row.capacity_minutes)))
        return parts.join("  ·  ")
    }

    Label {
        x: 18
        y: 15
        text: chart.title
        color: chart.win.text
        font.pixelSize: 15
        font.weight: Font.Medium
    }
    Row {
        anchors.right: parent.right
        anchors.rightMargin: 18
        y: 17
        spacing: 14
        Label {
            text: chart.subtitle
            textFormat: Text.PlainText
            color: chart.win.muted
            font.pixelSize: 12
        }
        Row {
            spacing: 6
            Rectangle { width: 10; height: 10; radius: 2; color: theme.accent; anchors.verticalCenter: parent.verticalCenter }
            Label { text: qsTr("tracked"); color: chart.win.muted; font.pixelSize: 12 }
        }
        Row {
            spacing: 6
            Rectangle { width: 10; height: 10; radius: 2; color: "#45423b"; anchors.verticalCenter: parent.verticalCenter }
            Label { text: qsTr("planned"); color: chart.win.muted; font.pixelSize: 12 }
        }
    }
    Item {
        id: plot
        anchors.fill: parent
        anchors.leftMargin: 58
        anchors.rightMargin: 18
        anchors.topMargin: 50
        anchors.bottomMargin: 34
        Repeater {
            model: chart.axisLines + 1
            Item {
                required property int index
                width: plot.width
                height: 1
                y: index * (plot.height - 1) / chart.axisLines
                Rectangle {
                    width: parent.width
                    height: 1
                    color: "#302e29"
                }
                Label {
                    x: -52
                    y: -7
                    width: 44
                    horizontalAlignment: Text.AlignRight
                    color: chart.win.muted
                    font.family: chart.win.mono
                    font.pixelSize: 10
                    text: chart.axisText(Math.round((chart.axisLines - index) * chart.maximum / chart.axisLines))
                }
            }
        }
        Row {
            anchors.fill: parent
            spacing: 2
            Repeater {
                model: chart.days
                Item {
                    id: column
                    required property var modelData
                    required property int index
                    width: (plot.width - Math.max(0, chart.days.length - 1) * 2) / Math.max(1, chart.days.length)
                    height: plot.height
                    Rectangle {
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Math.max(3, Math.min(42, parent.width - 3))
                        height: plot.height * Number(column.modelData.planned_minutes || 0) / chart.maximum
                        radius: 3
                        color: "#45423b"
                    }
                    Rectangle {
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Math.max(2, Math.min(30, parent.width - 7))
                        height: plot.height * Number(column.modelData.minutes || 0) / chart.maximum
                        radius: 3
                        color: theme.accent
                    }
                    Label {
                        anchors.top: parent.bottom
                        anchors.topMargin: 8
                        anchors.horizontalCenter: parent.horizontalCenter
                        color: chart.win.muted
                        font.family: chart.win.mono
                        font.pixelSize: 10
                        text: chart.dayLabel(column.index)
                    }
                    MouseArea {
                        anchors.fill: parent
                        hoverEnabled: true
                        onEntered: {
                            if (!chart.tip)
                                return
                            chart.tip.text = chart.tooltip(column.modelData)
                            chart.tip.visible = true
                            var p = mapToItem(chart.win.contentItem, width / 2, 0)
                            chart.tip.x = Math.max(230, Math.min(chart.win.width - chart.tip.width - 10,
                                p.x - chart.tip.width / 2))
                            chart.tip.y = Math.max(10, p.y - chart.tip.height - 8)
                        }
                        onExited: {
                            if (chart.tip)
                                chart.tip.visible = false
                        }
                    }
                }
            }
        }
    }
}
