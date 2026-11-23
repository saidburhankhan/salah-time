import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
import "ResourceTime.js" as Time

// One resource in the drawer: a note, a link (Open) or a reminder (done box).
Rectangle {
    id: row
    required property var win
    required property var resource
    readonly property string kind: String(resource.kind || "")
    readonly property bool done: !!resource.done
    readonly property bool overdue: kind === "reminder" && Time.isOverdue(resource.remind_at, done)
    readonly property color urgent: "#c4876b"
    signal editRequested()
    signal removeRequested()

    implicitHeight: content.implicitHeight + 24
    radius: theme.radius
    color: win.card
    border.color: overdue ? Qt.rgba(urgent.r, urgent.g, urgent.b, 0.55) : win.border

    RowLayout {
        id: content
        anchors.fill: parent
        anchors.margins: 12
        spacing: 10

        Rectangle {
            visible: row.kind === "reminder"
            objectName: "resourceDone_" + row.resource.id
            Layout.alignment: Qt.AlignTop
            width: 22
            height: 22
            radius: Math.min(6, theme.radius)
            color: row.done ? row.win.selectedFill : "transparent"
            border.color: row.done ? row.win.selectedBorder : "#625e55"
            border.width: 2
            Label {
                anchors.centerIn: parent
                visible: row.done
                text: "✓"
                color: row.win.selectedText
                font.weight: Font.Bold
            }
            MouseArea {
                anchors.fill: parent
                anchors.margins: -5
                cursorShape: Qt.PointingHandCursor
                onClicked: backend.editResource(row.resource.id, { done: !row.done })
            }
            Accessible.role: Accessible.CheckBox
            Accessible.checked: row.done
            Accessible.name: qsTr("Done")
        }
        Label {
            visible: row.kind !== "reminder"
            Layout.alignment: Qt.AlignTop
            text: row.kind === "link" ? "↗" : "¶"
            color: row.win.muted
            font.pixelSize: 15
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 4
            Label {
                visible: text !== ""
                Layout.fillWidth: true
                text: row.kind === "note" ? String(row.resource.body || "")
                    : String(row.resource.title || "")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                maximumLineCount: row.kind === "note" ? 8 : 3
                elide: Text.ElideRight
                color: row.done ? row.win.muted : row.win.text
                font.pixelSize: 14
                font.strikeout: row.done
            }
            Label {
                visible: row.kind === "link"
                Layout.fillWidth: true
                text: String(row.resource.url || "")
                textFormat: Text.PlainText
                elide: Text.ElideMiddle
                color: row.win.selectedText
                font.family: row.win.mono
                font.pixelSize: 12
            }
            Label {
                visible: row.kind === "reminder"
                Layout.fillWidth: true
                text: (row.overdue ? qsTr("Overdue · ") : "") + Time.display(row.resource.remind_at)
                textFormat: Text.PlainText
                color: row.overdue ? row.urgent : row.win.muted
                font.family: row.win.mono
                font.pixelSize: 12
            }
            RowLayout {
                spacing: 6
                AppButton {
                    visible: row.kind === "link"
                    objectName: "resourceOpen_" + row.resource.id
                    win: row.win
                    text: qsTr("Open")
                    primary: true
                    implicitHeight: 30
                    enabled: Time.validUrl(row.resource.url)
                    onClicked: Qt.openUrlExternally(String(row.resource.url))
                }
                AppButton {
                    objectName: "resourceEdit_" + row.resource.id
                    win: row.win
                    text: qsTr("Edit")
                    implicitHeight: 30
                    onClicked: row.editRequested()
                }
                AppButton {
                    objectName: "resourceRemove_" + row.resource.id
                    win: row.win
                    text: qsTr("Remove")
                    implicitHeight: 30
                    onClicked: row.removeRequested()
                }
            }
        }
    }
}
