import QtQuick
import qs.Commons
import qs.Ui
import "ResourceLogic.js" as ResourceLogic

Column {
  id: section

  required property var host
  required property string fontFamily
  property bool expanded: false
  property string kind: ""
  property var entries: []
  property int editingIndex: -1
  property bool unsavedNote: false
  readonly property string value: kind === "note" ? noteDraft.text : draft.text
  readonly property bool textFocus: draft.activeFocus || noteDraft.textFocus ||
    reminderDate.activeFocus || reminderTime.activeFocus
  readonly property bool hasInput: kind !== "" && (editingIndex >= 0 || value.trim() !== "" ||
    (kind === "reminder" && (reminderDate.text.trim() !== "" || reminderTime.text.trim() !== "")))
  readonly property var draftParams: ResourceLogic.resourceDraft(kind, value, reminderDate.text, reminderTime.text)

  spacing: Style.space(3)

  function clearDraft() {
    kind = ""
    editingIndex = -1
    unsavedNote = false
    draft.clear()
    noteDraft.text = ""
    reminderDate.clear()
    reminderTime.clear()
  }

  function reset() {
    expanded = false
    entries = []
    clearDraft()
  }

  function cancelLayer() {
    if (!expanded) return false
    if (kind) clearDraft()
    else expanded = false
    return true
  }

  function editNote(index) {
    if (editingIndex !== index && !leaveDraft()) return
    kind = "note"
    editingIndex = index
    noteDraft.text = entries[index].body
    noteDraft.focusInput()
  }

  function leaveDraft() {
    if (kind !== "note") return commitDraft()
    var original = editingIndex < 0 ? "" : entries[editingIndex].body
    if (noteDraft.text !== original) {
      unsavedNote = true
      noteDraft.focusInput()
      return false
    }
    clearDraft()
    return true
  }

  function commitDraft() {
    if (!hasInput) return true
    if (!draftParams) {
      expanded = true
      return false
    }
    var next = entries.slice()
    if (editingIndex < 0) next.push(draftParams)
    else next[editingIndex] = draftParams
    entries = next
    clearDraft()
    return true
  }

  component ResourceButton: Button {
    height: Style.space(23)
    bordered: true
    foreground: section.host.foreground
    fontFamily: section.fontFamily
    fontSize: Style.font.bodySmall
    horizontalPadding: Style.space(5)
    verticalPadding: 0
  }

  component DraftField: TextField {
    height: Style.space(26)
    foreground: section.host.foreground
    font.family: section.fontFamily
    font.pixelSize: Style.font.bodySmall
    verticalPadding: 0
    onAccepted: section.commitDraft()
    Keys.onEscapePressed: function(event) {
      section.clearDraft()
      event.accepted = true
    }
  }

  ResourceButton {
    text: section.entries.length
      ? section.host.tr.t("creation.resourcesCount").arg(section.entries.length)
      : section.host.tr.t("creation.resources")
    selected: section.expanded
    onClicked: section.expanded = !section.expanded
  }

  Row {
    visible: section.expanded
    spacing: Style.space(4)

    Repeater {
      model: [
        { kind: "note", label: "resource.addNote" },
        { kind: "link", label: "resource.addLink" },
        { kind: "reminder", label: "resource.addReminder" }
      ]

      ResourceButton {
        required property var modelData
        text: section.host.tr.t(modelData.label)
        selected: section.kind === modelData.kind
        onClicked: {
          if (section.kind !== modelData.kind) {
            if (!section.leaveDraft()) return
            section.kind = modelData.kind
          }
          if (section.kind === "note") noteDraft.focusInput()
          else draft.forceActiveFocus()
        }
      }
    }
  }

  Column {
    visible: section.expanded && section.kind !== ""
    width: parent.width
    spacing: Style.space(3)

    Row {
      width: parent.width
      spacing: Style.space(4)

      DraftField {
        id: draft
        visible: section.kind !== "note"
        width: Math.max(0, parent.width - attach.width - cancel.width - parent.spacing * 2)
        maximumLength: section.kind === "link" ? 2000 : 200
        placeholderText: section.host.tr.t(section.kind === "link" ? "resource.url" : "resource.reminderTitle")
      }

      NoteInput {
        id: noteDraft
        visible: section.kind === "note"
        width: draft.width
        placeholderText: section.host.tr.t("resource.note")
        foreground: section.host.foreground
        fontFamily: section.fontFamily
        onTextChanged: section.unsavedNote = false
        onAccepted: section.commitDraft()
        onCanceled: section.clearDraft()
      }

      ResourceButton {
        id: cancel
        text: section.host.tr.t("session.cancel")
        onClicked: section.clearDraft()
      }

      ResourceButton {
        id: attach
        text: section.host.tr.t(section.editingIndex < 0 ? "creation.attach" : "session.save")
        enabled: !!section.draftParams
        opacity: enabled ? 1 : 0.45
        onClicked: section.commitDraft()
      }
    }

    Row {
      visible: section.kind === "reminder"
      width: parent.width
      spacing: Style.space(4)

      DraftField {
        id: reminderDate
        width: (parent.width - parent.spacing) / 2
        maximumLength: 10
        placeholderText: "YYYY-MM-DD"
      }

      DraftField {
        id: reminderTime
        width: reminderDate.width
        maximumLength: 5
        placeholderText: "HH:MM"
      }
    }

    Text {
      visible: section.unsavedNote || !section.draftParams && (section.kind !== "note" || section.hasInput)
      width: parent.width
      textFormat: Text.PlainText
      text: section.host.tr.t(section.unsavedNote ? "session.unsavedNote" : section.kind === "link"
        ? "resource.invalidUrl" : section.kind === "note" ? "resource.noteHint" : "resource.reminderTimeHint")
      color: section.host.mutedForeground
      font.family: section.fontFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
    }
  }

  Flickable {
    visible: section.expanded && section.entries.length > 0
    width: parent.width
    height: Math.min(contentHeight, Style.space(78))
    contentWidth: width
    contentHeight: drafts.implicitHeight
    interactive: contentHeight > height
    flickableDirection: Flickable.VerticalFlick
    boundsBehavior: Flickable.StopAtBounds
    clip: true

    Column {
      id: drafts
      width: section.width
      spacing: Style.space(3)

      Repeater {
        model: section.entries

        Row {
          id: summary
          required property var modelData
          required property int index
          readonly property bool isNote: modelData.kind === "note"
          visible: index !== section.editingIndex
          width: drafts.width
          spacing: Style.space(4)

          Text {
            width: Math.max(0, parent.width - remove.width - parent.spacing
              - (summary.isNote ? edit.width + parent.spacing : 0))
            anchors.verticalCenter: parent.verticalCenter
            textFormat: Text.PlainText
            text: modelData.body || modelData.url || section.host.tr.t("reminder.subjectTime")
              .arg(modelData.title || section.host.tr.t("reminder.untitled"))
              .arg(section.host.resourceDateTime(modelData.remind_at))
            color: section.host.foreground
            font.family: section.fontFamily
            font.pixelSize: Style.font.bodySmall
            elide: Text.ElideRight
            maximumLineCount: 1
            MouseArea {
              anchors.fill: parent
              enabled: summary.isNote
              cursorShape: Qt.PointingHandCursor
              onClicked: section.editNote(summary.index)
            }
          }

          ResourceButton {
            id: edit
            visible: summary.isNote
            text: "✎"
            tooltipText: section.host.tr.t("session.editNote")
            onClicked: section.editNote(summary.index)
          }

          ResourceButton {
            id: remove
            text: section.host.tr.t("session.remove")
            tooltipText: section.host.tr.t("creation.removeAttachment")
            onClicked: {
              var remaining = section.entries.slice()
              remaining.splice(index, 1)
              section.entries = remaining
              if (index < section.editingIndex) section.editingIndex--
            }
          }
        }
      }
    }
  }
}
