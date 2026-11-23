import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.Commons
import qs.Commons as Commons

// A separate, card-sized surface keeps the main panel and its bar click
// targets usable while the note field is open.
PanelWindow {
  id: popover

  required property Item anchorItem
  required property var host
  readonly property var barWindow: anchorItem.QsWindow.window
  readonly property string edge: host.bar ? host.bar.position : "top"
  readonly property bool side: edge === "left" || edge === "right"
  readonly property int inset: Style.space(8)
  readonly property int barSize: host.bar && screen && typeof host.bar.barSizeFor === "function"
    ? host.bar.barSizeFor(screen.name) : host.barSize
  readonly property point center: {
    watcher.transform
    return barWindow ? anchorItem.mapToItem(barWindow.contentItem, anchorItem.width / 2,
      anchorItem.height / 2) : Qt.point(0, 0)
  }

  screen: barWindow ? barWindow.screen : null
  visible: host.noteSource === "bar" && !!barWindow
  implicitWidth: Math.min(Style.space(320), screen ? screen.width : Style.space(320))
  implicitHeight: editor.implicitHeight + inset * 2
  exclusionMode: ExclusionMode.Ignore
  color: "transparent"
  WlrLayershell.namespace: "salah-session-note"
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: !visible ? WlrKeyboardFocus.None
    : (focusSettled ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.Exclusive)
  property bool focusSettled: false

  anchors.left: edge !== "right"
  anchors.right: edge === "right"
  anchors.top: edge !== "bottom"
  anchors.bottom: edge === "bottom"
  margins.left: side ? barSize + Style.gapsOut
    : Math.max(0, Math.min(center.x - implicitWidth / 2, (screen ? screen.width : 0) - implicitWidth))
  margins.right: side ? barSize + Style.gapsOut : 0
  margins.top: !side ? barSize + Style.gapsOut
    : Math.max(0, Math.min(center.y - implicitHeight / 2, (screen ? screen.height : 0) - implicitHeight))
  margins.bottom: !side ? barSize + Style.gapsOut : 0

  TransformWatcher {
    id: watcher
    a: popover.barWindow ? popover.barWindow.contentItem : null
    b: popover.anchorItem
  }

  onBackingWindowVisibleChanged: {
    if (backingWindowVisible) {
      Qt.callLater(editor.focusInput)
      settleFocus.restart()
    }
  }
  Timer {
    id: settleFocus
    interval: 75
    onTriggered: popover.focusSettled = true
  }

  Rectangle {
    anchors.fill: parent
    radius: Style.cornerRadius
    color: Commons.Color.popups.background
    border.width: Style.spacing.hairline
    border.color: Style.normalBorderFor(popover.host.foreground, Commons.Color.accent)
  }

  SessionNoteField {
    id: editor
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    anchors.margins: popover.inset
    host: popover.host
  }
}
