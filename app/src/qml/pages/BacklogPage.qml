import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../backlog"
import "../controls"

ColumnLayout {
    id: page
    objectName: "backlogPage"
    required property var win
    readonly property bool current: win.page === 5
    property string tagId: ""
    property string projectId: ""
    property var dragRows: []
    readonly property var liveRows: groupRows(backend.backlogItems, backend.epics, backend.tags)
    readonly property var rows: win.backlogDrag.active ? dragRows : liveRows

    spacing: 12

    onCurrentChanged: {
        if (current)
            load()
        else
            backend.closeBacklog()
    }

    function load() {
        backend.loadBacklog(tagId, projectId)
    }

    function selectTag(id) {
        tagId = id
        load()
    }

    function selectProject(id) {
        projectId = id
        load()
    }

    function groupRows(items, epics, tags) {
        var projects = new Map()
        for (var task of items) {
            var project = task.project
            var projectId = project ? project.id : ""
            var group = projects.get(projectId)
            if (!group) {
                group = { project: project, epics: new Map() }
                projects.set(projectId, group)
            }
            var epicId = task.epic_id || ""
            if (!group.epics.has(epicId))
                group.epics.set(epicId, [])
            group.epics.get(epicId).push(task)
        }

        var orderedProjects = Array.from(projects.values()).sort(function(a, b) {
            if (!a.project) return 1
            if (!b.project) return -1
            return a.project.name.localeCompare(b.project.name)
        })
        var rows = []
        for (var group of orderedProjects) {
            var project = group.project
            var tagNames = project ? tags.filter(function(tag) {
                return (project.tag_ids || []).indexOf(tag.id) >= 0
            }).map(function(tag) { return tag.name }) : []
            rows.push({ kind: "project", title: project ? project.name : qsTr("No project"),
                        projectId: project ? project.id : "", color: project ? project.color : "",
                        tags: tagNames.join(" · ") })
            var byEpic = new Map(epics.map(function(epic) { return [epic.id, epic] }))
            var orderedEpics = Array.from(group.epics.keys()).sort(function(a, b) {
                if (!a) return 1
                if (!b) return -1
                return (byEpic.get(a)?.name || "").localeCompare(byEpic.get(b)?.name || "")
            })
            for (var epicId of orderedEpics) {
                var epic = byEpic.get(epicId)
                if (project)
                    rows.push({ kind: "epic", title: epic ? epic.name : (epicId ? qsTr("Epic") : qsTr("No epic")),
                                epicId: epic ? epic.id : "" })
                for (var task of group.epics.get(epicId))
                    rows.push({ kind: "task", task: task })
            }
        }
        return rows
    }

    function reorderTarget(point, task) {
        var p = tasks.mapFromItem(win.contentItem, point)
        if (!tasks.contains(p))
            return null
        var index = tasks.indexAt(p.x + tasks.contentX, p.y + tasks.contentY)
        var entry = rows[index]
        if (!entry || entry.kind !== "task" || entry.task.id === task.id
                || entry.task.project_id !== task.project_id || entry.task.epic_id !== task.epic_id)
            return null
        var row = tasks.itemAtIndex(index)
        if (!row)
            return null
        var before = row.mapFromItem(win.contentItem, point).y < row.height / 2
        var neighbor = rows[index + (before ? -1 : 1)]
        if (neighbor && neighbor.kind === "task" && neighbor.task.id === task.id)
            return null
        return { id: entry.task.id, before: before }
    }

    function scrollForDrag(point) {
        var p = tasks.mapFromItem(win.contentItem, point)
        if (!tasks.contains(p))
            return
        var delta = p.y < 36 ? -14 : p.y > tasks.height - 36 ? 14 : 0
        if (delta !== 0)
            tasks.contentY = Math.max(tasks.originY,
                Math.min(tasks.originY + Math.max(0, tasks.contentHeight - tasks.height), tasks.contentY + delta))
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 14
        Label {
            Layout.preferredWidth: 64
            text: qsTr("Tags")
            color: page.win.muted
        }
        Flow {
            Layout.fillWidth: true
            spacing: 6
            Repeater {
                model: [{ id: "", name: qsTr("All tags") }, { id: "none", name: qsTr("No tag") }]
                    .concat(backend.tags || [])
                ChoiceChip {
                    required property var modelData
                    objectName: "backlogTag_" + modelData.id
                    win: page.win
                    text: modelData.name
                    dotColor: modelData.color || "transparent"
                    selected: page.tagId === modelData.id
                    enabled: !backend.offlineMode
                    implicitHeight: 32
                    onClicked: page.selectTag(modelData.id)
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 14
        Label {
            Layout.preferredWidth: 64
            text: qsTr("Projects")
            color: page.win.muted
        }
        Flow {
            Layout.fillWidth: true
            spacing: 6
            Repeater {
                model: [{ id: "", name: qsTr("All projects") }, { id: "none", name: qsTr("No project") }]
                    .concat(backend.projects || [])
                ChoiceChip {
                    required property var modelData
                    objectName: "backlogProject_" + modelData.id
                    win: page.win
                    text: modelData.name
                    dotColor: modelData.color || "transparent"
                    selected: page.projectId === modelData.id
                    enabled: !backend.offlineMode
                    implicitHeight: 32
                    onClicked: page.selectProject(modelData.id)
                }
            }
        }
    }

    Rectangle {
        Layout.fillWidth: true
        Layout.topMargin: 3
        height: 1
        color: page.win.border
    }

    Item {
        Layout.fillWidth: true
        Layout.fillHeight: true
        ListView {
            id: tasks
            objectName: "backlogTaskList"
            anchors.fill: parent
            clip: true
            spacing: 5
            model: page.rows
            ScrollBar.vertical: ScrollBar {}
            delegate: Loader {
                required property var modelData
                property var entry: modelData
                width: tasks.width - 15
                sourceComponent: entry.kind === "task" ? taskRow
                    : entry.kind === "epic" ? epicHeading : projectHeading
            }
        }
        Label {
            anchors.centerIn: parent
            visible: page.rows.length === 0 || backend.offlineMode
            text: backend.offlineMode ? qsTr("The Backlog needs the Salah daemon.")
                : backend.backlogLoading ? qsTr("Loading backlog…")
                : page.tagId || page.projectId ? qsTr("No tasks match these filters.")
                : qsTr("No tasks in the backlog.")
            color: page.win.muted
        }
    }

    Component {
        id: projectHeading
        RowLayout {
            height: 38
            spacing: 9
            Rectangle {
                objectName: "backlogProjectDot_" + entry.projectId
                implicitWidth: 10
                implicitHeight: 10
                radius: 5
                color: page.win.projectById(entry.projectId)?.color || entry.color || page.win.muted
            }
            Label {
                text: entry.title
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: page.win.text
                font.pixelSize: 16
                font.weight: Font.DemiBold
            }
            Label {
                Layout.fillWidth: true
                text: entry.tags
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: page.win.muted
                font.pixelSize: 12
            }
        }
    }

    Component {
        id: epicHeading
        Item {
            height: 30
            Label {
                anchors.left: parent.left
                anchors.leftMargin: 16
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(implicitWidth, parent.width - 24)
                text: entry.title + (entry.epicId ? "  ↗" : "")
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: entry.epicId ? page.win.selectedText : page.win.muted
                font.pixelSize: 13
                Accessible.role: entry.epicId ? Accessible.Button : Accessible.StaticText
                Accessible.name: entry.title
                TapHandler {
                    enabled: entry.epicId !== ""
                    onTapped: page.win.openEpic(entry.epicId)
                }
                HoverHandler {
                    enabled: entry.epicId !== ""
                    cursorShape: Qt.PointingHandCursor
                }
            }
        }
    }

    Component {
        id: taskRow
        BacklogRow {
            width: parent.width
            win: page.win
            task: entry.task
        }
    }
}
