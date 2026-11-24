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
  readonly property bool textFocus: draft.activeFocus || reminderDate.activeFocus || reminderTime.activeFocus
  readonly property bool hasInput: kind !== "" && (draft.text.trim() !== "" ||
    (kind === "reminder" && (reminderDate.text.trim() !== "" || reminderTime.text.trim() !== "")))
  readonly property var draftParams: ResourceLogic.resourceDraft(kind, draft.text, reminderDate.text, reminderTime.text)

  spacing: Style.space(3)

  function clearDraft() {
    kind = ""
    draft.clear()
    reminderDate.clear()
    reminderTime.clear()
  }

  function reset() {
    expanded = false
    entries = []
    clearDraft()
  }

  function commitDraft() {
    if (!hasInput) return true
    if (!draftParams) {
      expanded = true
      return false
    }
    entries = entries.concat(draftParams)
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
            if (!section.commitDraft()) return
            section.kind = modelData.kind
          }
          draft.forceActiveFocus()
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
        width: Math.max(0, parent.width - attach.width - parent.spacing)
        maximumLength: section.kind === "note" ? 5000 : section.kind === "link" ? 2000 : 200
        placeholderText: section.host.tr.t(section.kind === "note" ? "resource.note"
          : section.kind === "link" ? "resource.url" : "resource.reminderTitle")
      }

      ResourceButton {
        id: attach
        height: draft.height
        text: section.host.tr.t("creation.attach")
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
      visible: !section.draftParams && section.kind !== "note"
      width: parent.width
      textFormat: Text.PlainText
      text: section.host.tr.t(section.kind === "link" ? "resource.invalidUrl" : "resource.reminderTimeHint")
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
          required property var modelData
          required property int index
          width: drafts.width
          spacing: Style.space(4)

          Text {
            width: Math.max(0, parent.width - remove.width - parent.spacing)
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
          }

          ResourceButton {
            id: remove
            text: section.host.tr.t("session.remove")
            tooltipText: section.host.tr.t("creation.removeAttachment")
            onClicked: {
              var remaining = section.entries.slice()
              remaining.splice(index, 1)
              section.entries = remaining
            }
          }
        }
      }
    }
  }
}
