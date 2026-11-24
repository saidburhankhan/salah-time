import QtQuick
import QtQml.Models
import Quickshell
import Quickshell.Wayland
import qs.Ui
import "PanelLogic.js" as PanelLogic

KeyboardPanel {
  id: panel

  readonly property var barState: bar && bar.shell ? bar.shell.bar : null

  // Omarchy 4.5's KeyboardPanel creates unmasked dismissal windows on other
  // outputs. Keep its focus and popup handling and mask just those windows.
  // The namespace identifies the windows even if their declaration moves.
  function dismissalWindows() {
    var windows = []
    for (var i = 0; i < data.length; i++) {
      var object = data[i]
      var instances = object && object.instances
      if (!instances) continue
      for (var j = 0; j < instances.length; j++) {
        var window = instances[j]
        if (!window || !("exclusionMode" in window)) continue
        var layer = window.WlrLayershell
        if (layer && layer.namespace === "omarchy-keyboard-panel-dismiss") windows.push(window)
      }
    }
    return windows
  }

  // Not a child: KeyboardPanel's default property is `contentItem` (Items
  // only), so a non-visual helper must live in its own property.
  property Instantiator dismissalMasks: Instantiator {
    model: panel.open && panel.bar ? panel.dismissalWindows() : []

    delegate: QtObject {
      id: dismissal
      required property var modelData
      readonly property var screen: modelData.screen
      readonly property real thickness: {
        if (!screen || (panel.barState && panel.barState.barHidden)) return 0
        if (panel.barState && typeof panel.barState.barSizeFor === "function")
          return panel.barState.barSizeFor(screen.name)
        if (typeof panel.bar.barSizeFor === "function") return panel.bar.barSizeFor(screen.name)
        return panel.bar.barSize
      }
      readonly property var strip: PanelLogic.barStrip(panel.bar.position,
        screen ? screen.width : 0, screen ? screen.height : 0, thickness)
      property bool unmasked: false
      Component.onCompleted: unmasked = !modelData.mask

      property Region inputRegion: Region {
        width: dismissal.screen ? dismissal.screen.width : 0
        height: dismissal.screen ? dismissal.screen.height : 0

        Region {
          intersection: Intersection.Subtract
          x: dismissal.strip.x
          y: dismissal.strip.y
          width: dismissal.strip.width
          height: dismissal.strip.height
        }
      }

      property Binding maskBinding: Binding {
        target: dismissal.modelData
        property: "mask"
        value: dismissal.inputRegion
        when: dismissal.unmasked && dismissal.thickness > 0
      }
    }
  }
}
