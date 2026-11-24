import QtQuick
import QtQuick.Layouts
import QtQuick.Shapes

Item {
    id: sidebarIcon
    property var win
    property string kind: ""
    property color iconColor: win ? win.muted : "#8f8a80"
    width: 22
    height: 22
    implicitWidth: 22
    implicitHeight: 22
    Layout.preferredWidth: 22
    Layout.preferredHeight: 22
    Shape {
        anchors.centerIn: parent
        width: 18
        height: 18
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: sidebarIcon.iconColor
            strokeWidth: 1.5
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathSvg {
                path: sidebarIcon.kind === "today"
                    ? "M3 5.5 L15 5.5 M6 2.5 L6 5 M12 2.5 L12 5 M4.5 3.5 L13.5 3.5 Q15 3.5 15 5 L15 14 Q15 15.5 13.5 15.5 L4.5 15.5 Q3 15.5 3 14 L3 5 Q3 3.5 4.5 3.5"
                    : sidebarIcon.kind === "backlog"
                        ? "M3 4 L15 4 L15 14 L3 14 Z M3 10 L6.5 10 L7.5 11.5 L10.5 11.5 L11.5 10 L15 10"
                    : sidebarIcon.kind === "routines"
                        ? "M14.5 5.5 L14.5 2.5 M14.5 5.5 L11.5 5.5 M14.3 5.3 A6.5 6.5 0 1 0 15 11"
                        : sidebarIcon.kind === "statistics"
                            ? "M3.5 15 L3.5 10 L6.5 10 L6.5 15 Z M7.5 15 L7.5 6.5 L10.5 6.5 L10.5 15 Z M11.5 15 L11.5 3 L14.5 3 L14.5 15 Z"
                            : sidebarIcon.kind === "insights"
                                ? "M2.5 3 L2.5 15 L15.5 15 M5 11 L8.5 7 L11 9 L15 4"
                            : sidebarIcon.kind === "settings"
                                ? "M3 5 L15 5 M3 9 L15 9 M3 13 L15 13 M6 3 L6 7 M12 7 L12 11 M8 11 L8 15"
                                : sidebarIcon.kind === "goals"
                                    ? "M9 2.5 A6.5 6.5 0 1 0 9 15.5 A6.5 6.5 0 1 0 9 2.5 M9 5.8 A3.2 3.2 0 1 0 9 12.2 A3.2 3.2 0 1 0 9 5.8 M9 8.6 L9 9.4"
                                : sidebarIcon.kind === "review"
                                    ? "M4 2.5 L14 2.5 L14 15.5 L4 15.5 Z M6.5 6 L11.5 6 M6.5 9 L11.5 9 M6.5 12 L9.5 12"
                                : "M9 16 C9 16 14 11.5 14 7.5 A5 5 0 1 0 4 7.5 C4 11.5 9 16 9 16 Z M9 9.5 A2 2 0 1 0 9 5.5 A2 2 0 1 0 9 9.5"
            }
        }
    }
}
