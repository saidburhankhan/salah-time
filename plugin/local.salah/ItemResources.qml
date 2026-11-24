import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "ResourceLogic.js" as ResourceLogic

Column {
  id: section

  required property var host
  required property string itemId
  required property string fontFamily
  readonly property var transport: host.resourceClient
  readonly property bool textFocus: draft.activeFocus
  readonly property var draftParams: ResourceLogic.quickResourceParams(itemId, addingKind, draft.text)
  property var rows: []
  property string requestId: ""
  property string loadError: ""
  property string actionError: ""
  property string addingKind: ""
  property string addOp: ""
  property var addAttempt: null

  spacing: Style.space(4)

  function reload() {
    requestId = transport.newOpId()
    transport.loadItemResources(itemId, requestId)
  }

  function toggleDraft(kind) {
    if (addOp) return
    var next = addingKind === kind ? "" : kind
    addingKind = next
    draft.clear()
    actionError = ""
    addAttempt = null
    if (next) draft.forceActiveFocus()
    else draft.focus = false
  }

  function save() {
    if (addOp || !draftParams) return
    var params = Object.assign({}, draftParams)
    var key = JSON.stringify(params)
    if (!addAttempt || addAttempt.key !== key)
      addAttempt = { id: transport.newOpId(), key: key }
    actionError = ""
    addOp = addAttempt.id
    host.addResource(addOp, params)
  }

  function openLink(value) {
    var url = ResourceLogic.webUrl(value)
    if (url && !Qt.openUrlExternally(url)) actionError = host.tr.t("resource.openFailed")
  }

  Component.onCompleted: reload()

  Connections {
    target: section.transport

    function onResourcesLoaded(id, resources, error) {
      if (id !== section.requestId) return
      if (!resources || typeof resources !== "object" || typeof resources.length !== "number") {
        section.loadError = error || section.host.tr.t("resource.loadFailed")
        return
      }
      section.loadError = ""
      // A QML signal can deliver a QVariantList, which has length but is not a JS Array.
      var rows = Array.from(resources)
      if (JSON.stringify(rows) !== JSON.stringify(section.rows)) section.rows = rows
    }

    function onEntitiesChanged(scopes) {
      if (scopes.indexOf("resources") !== -1) section.reload()
    }

    function onTransportChanged(active) { section.reload() }

    function onMutationSucceeded(opId, kind, result) {
      if (opId === section.addOp) {
        section.addOp = ""
        section.toggleDraft(section.addingKind)
      }
      var changed = result && result.changed && result.changed.resources
      if (changed && Array.from(changed).some(resource => resource.item_id === section.itemId)) section.reload()
    }

    function onMutationFailed(opId, kind, message, data) {
      if (opId === section.addOp) section.addOp = ""
      if (kind === "resource.edit") section.reload()
    }
  }

  Connections {
    target: section.host
    function onReportChanged() {
      if (!section.transport.socketActive) section.reload()
    }
  }

  Item {
    width: parent.width
    implicitHeight: buttons.implicitHeight

    Text {
      anchors.left: parent.left
      anchors.right: buttons.left
      anchors.rightMargin: Style.space(4)
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: section.host.tr.t("resource.heading")
      color: section.host.mutedForeground
      font.family: section.fontFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
    }

    Row {
      id: buttons
      anchors.right: parent.right
      spacing: Style.space(4)

      Repeater {
        model: ["note", "link"]

        Button {
          required property string modelData
          height: Style.space(23)
          text: section.host.tr.t(modelData === "note" ? "resource.addNote" : "resource.addLink")
          enabled: !section.addOp
          selected: section.addingKind === modelData
          bordered: true
          foreground: section.host.foreground
          fontFamily: section.fontFamily
          fontSize: Style.font.bodySmall
          horizontalPadding: Style.space(5)
          verticalPadding: 0
          onClicked: section.toggleDraft(modelData)
        }
      }
    }
  }

  Row {
    visible: section.addingKind !== ""
    width: parent.width
    spacing: Style.space(4)

    TextField {
      id: draft
      width: Math.max(0, parent.width - saveButton.width - parent.spacing)
      height: Style.space(26)
      maximumLength: section.addingKind === "note" ? 5000 : 2000
      placeholderText: section.host.tr.t(section.addingKind === "note" ? "resource.note" : "resource.url")
      foreground: section.host.foreground
      font.family: section.fontFamily
      font.pixelSize: Style.font.bodySmall
      verticalPadding: 0
      readOnly: !!section.addOp
      onTextEdited: section.actionError = ""
      onAccepted: section.save()
      Keys.onEscapePressed: function(event) {
        section.toggleDraft(section.addingKind)
        event.accepted = true
      }
    }

    Button {
      id: saveButton
      height: draft.height
      enabled: !section.addOp && !!section.draftParams
      opacity: enabled ? 1 : 0.45
      text: section.host.tr.t("session.save")
      bordered: true
      foreground: section.host.foreground
      fontFamily: section.fontFamily
      fontSize: Style.font.bodySmall
      verticalPadding: 0
      onClicked: section.save()
    }
  }

  Text {
    visible: section.addingKind === "link" && !section.draftParams
    width: parent.width
    textFormat: Text.PlainText
    text: section.host.tr.t("resource.invalidUrl")
    color: section.host.mutedForeground
    font.family: section.fontFamily
    font.pixelSize: Style.font.caption
    elide: Text.ElideRight
  }

  Flickable {
    width: parent.width
    height: Math.min(contentHeight, Style.space(136))
    visible: height > 0
    contentWidth: width
    contentHeight: entries.implicitHeight
    flickableDirection: Flickable.VerticalFlick
    interactive: contentHeight > height
    boundsBehavior: Flickable.StopAtBounds
    clip: true

    Column {
      id: entries
      width: section.width
      spacing: Style.space(4)

      Repeater {
        model: section.rows

        Item {
          id: entry
          required property var modelData
          readonly property bool link: modelData.kind === "link"
          readonly property bool reminder: modelData.kind === "reminder"
          readonly property bool done: modelData.done === true
          readonly property string safeUrl: link ? ResourceLogic.webUrl(modelData.url) : ""
          width: entries.width
          implicitHeight: Math.max(lines.implicitHeight, link || reminder ? action.height : 0)

          Column {
            id: lines
            anchors.left: parent.left
            anchors.right: action.visible ? action.left : parent.right
            anchors.rightMargin: action.visible ? Style.space(5) : 0
            anchors.verticalCenter: parent.verticalCenter

            Text {
              visible: text !== ""
              width: parent.width
              textFormat: Text.PlainText
              text: entry.link ? (entry.modelData.title || entry.modelData.url || "")
                : (entry.modelData.title || (entry.reminder ? section.host.tr.t("reminder.untitled") : ""))
              color: entry.safeUrl ? Commons.Color.accent : section.host.foreground
              font.family: section.fontFamily
              font.pixelSize: Style.font.bodySmall
              font.strikeout: entry.reminder && entry.done
              elide: Text.ElideRight
            }

            Text {
              visible: text !== ""
              width: parent.width
              textFormat: Text.PlainText
              text: entry.link ? "" : (entry.modelData.body || "")
              color: section.host.foreground
              font.family: section.fontFamily
              font.pixelSize: Style.font.bodySmall
              wrapMode: Text.Wrap
            }

            Text {
              visible: text !== ""
              width: parent.width
              textFormat: Text.PlainText
              text: entry.reminder ? section.host.resourceDateTime(entry.modelData.remind_at) : ""
              color: section.host.mutedForeground
              font.family: section.fontFamily
              font.pixelSize: Style.font.caption
              elide: Text.ElideRight
            }
          }

          MouseArea {
            anchors.fill: lines
            enabled: !!entry.safeUrl
            cursorShape: Qt.PointingHandCursor
            onClicked: section.openLink(entry.safeUrl)
          }

          Button {
            id: action
            visible: entry.reminder || !!entry.safeUrl
            enabled: entry.link || (!entry.done && !section.host.pendingReminders[entry.modelData.id])
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: Style.space(26)
            height: Style.space(22)
            text: entry.link ? section.host.tr.t("resource.openSymbol") : section.host.tr.t("panel.checkSymbol")
            tooltipText: entry.link ? section.host.tr.t("resource.openLink")
              : section.host.tr.t(entry.done ? "reminder.done" : "reminder.markDone")
            opacity: enabled || entry.done ? 1 : 0.45
            selected: entry.reminder && entry.done
            bordered: true
            foreground: section.host.foreground
            fontFamily: section.fontFamily
            horizontalPadding: 0
            verticalPadding: 0
            onClicked: {
              if (entry.link) section.openLink(entry.safeUrl)
              else section.host.completeReminder(entry.modelData.id, section.itemId, entry.modelData.version)
            }
          }
        }
      }
    }
  }

  Text {
    visible: text !== ""
    width: parent.width
    textFormat: Text.PlainText
    text: section.actionError || section.loadError
    color: Commons.Color.urgent
    font.family: section.fontFamily
    font.pixelSize: Style.font.caption
    wrapMode: Text.Wrap
    maximumLineCount: 2
    elide: Text.ElideRight
  }
}
