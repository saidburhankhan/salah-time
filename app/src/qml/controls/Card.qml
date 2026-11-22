import QtQuick
import QtQuick.Layouts

Rectangle {
    property var win
    radius: theme.radius
    color: win ? win.panel : "#1b1a17"
    border.color: win ? win.border : "#34312a"
}
