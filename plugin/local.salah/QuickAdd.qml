import QtQuick
import qs.Commons
import qs.Ui
import "CreationLogic.js" as CreationLogic

// Enter submits a bare quick line directly. The sheet uses the parsed line
// and preserves its choices when it has attachments.
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
  readonly property bool textFocus: quickField.activeFocus || resourceDrafts.textFocus
  property var parsed: null
  property string parseError: ""
  property var minutesChoice: null
  property string blockChoice: ""
  property string whenChoice: ""
  property var tagChoice: null
  property string projectId: ""
  property string epicId: ""
  property string submitting: ""
  property var itemAttempt: null
  property var resourceBatch: null
  property bool awaitingItemId: false
  readonly property bool taskAdded: awaitingItemId || resourceBatch !== null
  readonly property var planning: host.planner ? host.planner.planning_block : null
  readonly property string today: planning ? planning.day : (host.planner ? host.planner.day : "")
  readonly property var schedule: CreationLogic.schedule(parsed, whenChoice, today)
  readonly property var fields: CreationLogic.prefill(parsed, minutesChoice, blockChoice,
    host.quickAddBlock, schedule.when === "tomorrow" ? "fajr" : (planning ? planning.start : ""))
  readonly property var projects: host.projects
  readonly property var tagIds: CreationLogic.taskTagIds(
    CreationLogic.defaultTagIds(projects, projectId), tagChoice, host.tags)
  readonly property var projectEpics: CreationLogic.epicsForProject(host.epics, projectId)
  readonly property bool backlogAllowed: !!parsed && parsed.repeat === "none"

  function resetChoices() {
    minutesChoice = null
    blockChoice = ""
    whenChoice = ""
    tagChoice = null
    projectId = ""
    epicId = ""
  }

  function dismiss() {
    if (submitting) return
    resourceBatch = null
    awaitingItemId = false
    root.host.clearQuickAddPreset()
    quickField.clear()
    quickField.focus = false
  }

  function submit(backlog) {
    if (submitting || taskAdded || !parsed || (schedule.when === "tomorrow" && !schedule.date && !backlog)) return
    var params = CreationLogic.addParams(parsed, fields, projectId, backlog, schedule.date, epicId,
      tagChoice === null ? null : tagIds)
    if (!params || !resourceDrafts.commitDraft()) return
    itemAttempt = CreationLogic.itemAttempt(itemAttempt, params, transport.newOpId)
    writeItem()
  }

  function writeItem() {
    submitting = host.beginOp("add", "", "", itemAttempt.id)
    transport.rpcMutation(submitting, "item.add", Object.assign({}, itemAttempt.params), "add")
  }

  function completeItem(result) {
    if (!resourceDrafts.entries.length) {
      dismiss()
      return
    }
    var item = CreationLogic.addedItem(result)
    awaitingItemId = !item || !item.id
    if (awaitingItemId) return
    resourceBatch = CreationLogic.resourceBatch(item.id, resourceDrafts.entries, transport.newOpId)
    writeNextResource()
  }

  function writeNextResource() {
    var write = resourceBatch.pending[0]
    if (write) {
      submitting = write.id
      transport.rpcMutation(write.id, "resource.add", Object.assign({}, write.params), "creationResource")
    } else if (resourceBatch.failed.length === 0) {
      dismiss()
    }
  }

  function retryResources() {
    if (submitting) return
    if (awaitingItemId) writeItem()
    else {
      resourceBatch = CreationLogic.retryResourceBatch(resourceBatch)
      writeNextResource()
    }
  }

  onProjectsChanged: {
    if (projectId && !projects.some(project => project.id === projectId)) projectId = ""
  }
  onProjectIdChanged: {
    epicId = ""
    tagChoice = null
  }
  onProjectEpicsChanged: epicId = CreationLogic.validEpicId(host.epics, projectId, epicId)
  onExpandedChanged: if (expanded) {
    transport.loadProjects()
    transport.loadTags()
    transport.loadEpics()
  }

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
      if (root.expanded) {
        if (!active) root.transport.loadTags()
        parseTimer.restart()
      }
    }
    function onMutationSucceeded(opId, kind, result) {
      if (opId !== root.submitting) return
      root.submitting = ""
      if (kind === "add") root.completeItem(result)
      else if (kind === "creationResource") {
        root.resourceBatch = CreationLogic.advanceResourceBatch(root.resourceBatch, null)
        root.writeNextResource()
      }
    }
    function onMutationFailed(opId, kind, message, data) {
      if (opId !== root.submitting) return
      root.submitting = ""
      if (kind === "creationResource") {
        var detail = String(message || "").trim().replace(/[\r\n]+/g, " ").slice(0, 240)
        root.resourceBatch = CreationLogic.advanceResourceBatch(root.resourceBatch, detail)
        root.writeNextResource()
      }
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

    readOnly: !!root.submitting || root.taskAdded
    onTextChanged: {
      root.parsed = null
      root.parseError = ""
      if (text.trim()) parseTimer.restart()
      else {
        parseTimer.stop()
        root.resetChoices()
        root.itemAttempt = null
        resourceDrafts.reset()
      }
    }
    onAccepted: {
      if (root.submitting || root.taskAdded) return
      if (root.itemAttempt || resourceDrafts.entries.length || resourceDrafts.hasInput) {
        root.submit(root.schedule.when === "backlog")
      } else {
        root.host.addItem(text)
        text = ""
      }
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

  component SheetButton: Button {
    foreground: root.foreground
    bordered: true
    fontFamily: root.fontFamily
    fontSize: Style.font.bodySmall
    verticalPadding: Style.space(2)
    opacity: enabled ? 1 : 0.45
  }

  Column {
    visible: root.expanded && !root.taskAdded
    enabled: !root.submitting
    width: parent.width
    spacing: Style.space(3)

    ChipRow {
      visible: root.projects.length > 0
      caption: root.strings.t("creation.project")
      options: [{ id: "", name: root.strings.t("creation.noProject") }].concat(root.projects)
      selection: root.projectId
      onPicked: value => root.projectId = value
    }

    ChipRow {
      visible: root.projectEpics.length > 0
      caption: root.strings.t("creation.epic")
      options: [{ id: "", name: root.strings.t("creation.noEpic") }].concat(root.projectEpics)
      selection: root.epicId
      onPicked: value => root.epicId = value
    }

    ChipRow {
      visible: root.host.tags.length > 0
      caption: root.strings.t("creation.tags")
      options: root.host.tags
      selection: root.tagIds
      multiple: true
      onPicked: value => root.tagChoice = CreationLogic.toggleTag(root.tagIds, value)
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

    CreationResources {
      id: resourceDrafts
      width: parent.width
      host: root.host
      fontFamily: root.fontFamily
    }

    Row {
      anchors.right: parent.right
      spacing: Style.space(5)

      SheetButton {
        enabled: !!root.parsed
        text: root.strings.t("creation.add")
        onClicked: root.submit(root.schedule.when === "backlog")
      }

      SheetButton {
        enabled: root.backlogAllowed
        text: root.strings.t("creation.toBacklog")
        onClicked: root.submit(true)
      }
    }
  }

  Column {
    visible: root.taskAdded
    width: parent.width
    spacing: Style.space(3)

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: {
        if (root.submitting) return root.strings.t("creation.savingResources")
        if (root.awaitingItemId) return root.strings.t("creation.itemConfirmationFailed")
        var failed = root.resourceBatch && root.resourceBatch.failed[0]
        if (!failed) return ""
        var summary = root.strings.t("creation.resourceSaveFailed")
        return failed.error ? summary + " " + failed.error : summary
      }
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      wrapMode: Text.Wrap
      maximumLineCount: 3
      elide: Text.ElideRight
    }

    Row {
      visible: !root.submitting
      spacing: Style.space(5)

      SheetButton {
        text: root.strings.t("creation.retryResources")
        onClicked: root.retryResources()
      }

      SheetButton {
        text: root.strings.t("creation.dismiss")
        onClicked: root.dismiss()
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
