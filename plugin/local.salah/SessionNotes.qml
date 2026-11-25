import QtQuick
import qs.Commons
import "SessionLogic.js" as SessionLogic

Column {
  id: notes

  required property var host
  required property var session
  required property string surface
  readonly property string prefix: session ? surface + ":" + session.id : ""
  readonly property var entries: session && session.notes || []
  readonly property var entryIds: Array.from(entries, entry => entry.id)
  property var shownIds: []
  readonly property string appendSource: prefix + ":append"
  spacing: Style.space(3)
  onEntryIdsChanged: {
    var target = notes.host.noteTarget
    var noteId = target && target.params.note_id
    if (notes.host.noteSource === prefix + ":" + noteId
        && shownIds.includes(noteId) && !entryIds.includes(noteId)) notes.host.closeNote()
    if (JSON.stringify(entryIds) !== JSON.stringify(shownIds)) shownIds = entryIds
  }

  EditableNote {
    width: parent.width
    host: notes.host
    source: notes.prefix + ":main"
    target: SessionLogic.noteTarget("main", notes.session)
  }

  Repeater {
    model: notes.shownIds
    EditableNote {
      required property var modelData
      readonly property var entry: Array.from(notes.entries).find(row => row.id === modelData)
      width: notes.width
      host: notes.host
      source: notes.prefix + ":" + modelData
      target: SessionLogic.noteTarget("entry", notes.session, entry)
      timestamp: entry ? SessionLogic.noteTime(entry.at) : ""
    }
  }

  Loader {
    width: parent.width
    active: notes.host.noteSource === notes.appendSource && notes.prefix !== ""
    visible: active
    sourceComponent: SessionNoteField { host: notes.host }
  }
}
