import QtQuick
import qs.Commons
import qs.Ui
import "CreationLogic.js" as CreationLogic

// Enter submits the quick line directly. The inline sheet uses the core parse
// for the same line and retains explicit chip choices while the user types.
Column {
  id: root

  property var strings
  required property var transport
  property var host
  property string fontFamily
  property color foreground
  property color mutedForeground
  property string occurrenceHint: ""
  property string actionError: ""
  property alias quickField: quickField

  readonly property bool expanded: quickField.text.trim() !== ""
  property var parsed: null
  property string parseError: ""
  property var minutesChoice: null
  property string blockChoice: ""
  property string whenChoice: ""
  property string tagId: ""
  property string projectId: ""
  property string submitting: ""
  readonly property var planning: host.planner ? host.planner.planning_block : null
  readonly property string today: planning ? planning.day : (host.planner ? host.planner.day : "")
  readonly property var schedule: CreationLogic.schedule(parsed, whenChoice, today)
  readonly property var fields: CreationLogic.prefill(parsed, minutesChoice, blockChoice,
    host.quickAddBlock, schedule.when === "tomorrow" ? "fajr" : (planning ? planning.start : ""))
  readonly property var tags: CreationLogic.tagsForProjects(host.projects)
  readonly property var filteredProjects: CreationLogic.projectsForTag(host.projects, tagId)
  readonly property bool backlogAllowed: !!parsed && parsed.repeat === "none"

  function resetChoices() {
    minutesChoice = null
    blockChoice = ""
    whenChoice = ""
    tagId = ""
    projectId = ""
  }

  function dismiss() {
    submitting = ""
    root.host.clearQuickAddPreset()
    quickField.clear()
    quickField.focus = false
  }

  function submit(backlog) {
    if (submitting || !parsed || (schedule.when === "tomorrow" && !schedule.date && !backlog)) return
    var params = CreationLogic.addParams(parsed, fields, projectId, backlog, schedule.date)
    if (!params) return
    submitting = host.beginOp("add", "")
    transport.rpcMutation(submitting, "item.add", params, "add")
  }

  onTagsChanged: if (tagId && !tags.some(tag => tag.id === tagId)) tagId = ""
  onFilteredProjectsChanged: {
    if (projectId && !filteredProjects.some(project => project.id === projectId)) projectId = ""
  }
  onExpandedChanged: if (expanded) transport.loadProjects()

  Timer {
    id: parseTimer
    interval: 140
    onTriggered: if (root.expanded) root.transport.parseItem(quickField.text)
  }

  Connections {
    target: root.transport
    function onItemParsed(text, draft, error) {
      if (text !== quickField.text) return
      root.parsed = draft
      root.parseError = error
      if (draft && !root.backlogAllowed && root.whenChoice === "backlog") root.whenChoice = ""
    }
    function onTransportChanged(active) {
      if (root.expanded) parseTimer.restart()
    }
    function onMutationSucceeded(opId, kind, result) {
      if (opId === root.submitting) root.dismiss()
    }
    function onMutationFailed(opId, kind, message, data) {
      if (opId === root.submitting) root.submitting = ""
    }
  }

  width: parent ? parent.width : 0
  spacing: Style.space(3)

  TextField {
    id: quickField
    width: parent.width
    placeholderText: root.strings.t("panel.quickAddPlaceholder")
    foreground: root.foreground
    font.family: root.fontFamily
    maximumLength: 200

    readOnly: !!root.submitting
    onTextChanged: {
      root.parsed = null
      root.parseError = ""
      if (text.trim()) parseTimer.restart()
      else {
        parseTimer.stop()
        root.resetChoices()
      }
    }
    onAccepted: {
      if (root.submitting) return
      root.host.addItem(text)
      text = ""
    }

    Keys.onEscapePressed: function(event) {
      root.dismiss()
      event.accepted = true
    }
  }

  component ChipRow: CreationChipRow {
    width: root.width
    foreground: root.foreground
    mutedForeground: root.mutedForeground
    fontFamily: root.fontFamily
  }

  Column {
    visible: root.expanded
    enabled: !root.submitting
    width: parent.width
    spacing: Style.space(3)

    ChipRow {
      visible: root.tags.length > 0
      caption: root.strings.t("creation.tags")
      options: [{ id: "", name: root.strings.t("creation.all") }].concat(root.tags)
      selection: root.tagId
      onPicked: value => root.tagId = value
    }

    ChipRow {
      visible: root.host.projects.length > 0
      caption: root.strings.t("creation.project")
      options: [{ id: "", name: root.strings.t("creation.noProject") }].concat(root.filteredProjects)
      selection: root.projectId
      onPicked: value => root.projectId = value
    }

    ChipRow {
      caption: root.strings.t("session.minutes")
      options: CreationLogic.minuteChoices(root.parsed).map(minutes =>
        ({ id: minutes, name: root.host.formatMinutes(minutes) }))
      selection: root.fields.minutes
      onPicked: value => root.minutesChoice = value
    }

    ChipRow {
      caption: root.strings.t("creation.when")
      options: [
        { id: "today", name: root.strings.t("session.today") },
        { id: "tomorrow", name: root.strings.t("creation.tomorrow"), enabled: !!root.today },
        { id: "backlog", name: root.strings.t("creation.backlog"), enabled: root.backlogAllowed }
      ]
      selection: root.schedule.when
      onPicked: value => root.whenChoice = value
    }

    ChipRow {
      visible: root.schedule.when !== "backlog"
      caption: root.strings.t("creation.block")
      options: ["night", "fajr", "dhuhr", "asr", "maghrib", "isha"].map(block =>
        ({ id: block, name: root.host.prayerName(block, block) }))
      selection: root.fields.block
      onPicked: value => root.blockChoice = value
    }

    Row {
      anchors.right: parent.right
      spacing: Style.space(5)

      Button {
        enabled: !!root.parsed
        text: root.strings.t("creation.add")
        foreground: root.foreground
        bordered: true
        fontFamily: root.fontFamily
        fontSize: Style.font.bodySmall
        verticalPadding: Style.space(2)
        opacity: enabled ? 1 : 0.45
        onClicked: root.submit(root.schedule.when === "backlog")
      }

      Button {
        enabled: root.backlogAllowed
        text: root.strings.t("creation.toBacklog")
        foreground: root.foreground
        bordered: true
        fontFamily: root.fontFamily
        fontSize: Style.font.bodySmall
        verticalPadding: Style.space(2)
        opacity: enabled ? 1 : 0.45
        onClicked: root.submit(true)
      }
    }
  }

  Text {
    visible: !root.expanded && root.host.quickAddBlock !== ""
    width: parent.width
    textFormat: Text.PlainText
    text: root.strings.t("panel.addingTo").arg(root.host.prayerName(root.host.quickAddBlock, root.host.quickAddBlock))
    color: root.mutedForeground
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    elide: Text.ElideRight
  }

  Text {
    visible: root.occurrenceHint !== ""
    width: parent.width
    textFormat: Text.PlainText
    text: root.occurrenceHint
    color: root.mutedForeground
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    elide: Text.ElideRight
  }

  Text {
    visible: text !== ""
    width: parent.width
    textFormat: Text.PlainText
    text: root.parseError || root.actionError
    color: root.mutedForeground
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    wrapMode: Text.WrapAnywhere
    maximumLineCount: 2
    elide: Text.ElideRight
  }
}
