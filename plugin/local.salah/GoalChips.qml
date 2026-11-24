import QtQuick
import qs.Commons
import qs.Commons as Commons
import "GoalLogic.js" as GoalLogic

// Compact goal progress chips for the panel header. Renders nothing when the
// status has no goals.
Item {
  id: chips

  required property var host
  readonly property var goals: host.report && host.report.goals ? host.report.goals : []
  readonly property var summary: GoalLogic.goalChips(goals, {
    projects: host.projects,
    epics: host.epics,
    items: host.planner && host.planner.items ? host.planner.items : [],
    metrics: host.metricCatalogue,
    strings: {
      allWork: host.tr.t("goal.allWork"),
      task: host.tr.t("goal.task"),
      epic: host.tr.t("goal.epic"),
      project: host.tr.t("goal.project"),
      none: host.tr.t("goal.none"),
      sumOverflow: host.tr.t("goal.sumOverflow"),
      minuteUnit: host.tr.t("goal.minuteUnit")
    }
  })
  readonly property string fontFamily: host.bar ? host.bar.fontFamily : Style.font.family
  readonly property color tint: Qt.rgba(Commons.Color.accent.r, Commons.Color.accent.g, Commons.Color.accent.b, 0.7)

  visible: summary.chips.length > 0
  width: parent ? parent.width : 0
  implicitHeight: visible ? flow.implicitHeight : 0
  height: implicitHeight

  Flow {
    id: flow
    width: parent.width
    spacing: Style.space(4)

    Repeater {
      model: chips.summary.chips

      Rectangle {
        id: chip
        required property var modelData
        width: Math.min(flow.width, row.implicitWidth + Style.space(12))
        height: row.implicitHeight + bar.height + Style.space(8)
        radius: Style.cornerRadius
        color: Style.hoverFillFor(chips.host.foreground, Commons.Color.accent)
        border.width: Style.spacing.hairline
        border.color: Style.normalBorderFor(chips.host.foreground, Commons.Color.accent)

        Row {
          id: row
          anchors.left: parent.left
          anchors.leftMargin: Style.space(6)
          anchors.top: parent.top
          anchors.topMargin: Style.space(3)
          spacing: Style.space(4)

          Text {
            textFormat: Text.PlainText
            text: chip.modelData.period === "week" ? chips.host.tr.t("goal.weekMark") : ""
            visible: text !== ""
            color: chips.host.mutedForeground
            font.family: chips.fontFamily
            font.pixelSize: Style.font.bodySmall
          }

          Text {
            width: Math.min(implicitWidth, Style.space(110))
            textFormat: Text.PlainText
            text: chip.modelData.label
            color: chips.host.mutedForeground
            font.family: chips.fontFamily
            font.pixelSize: Style.font.bodySmall
            elide: Text.ElideRight
          }

          Text {
            textFormat: Text.PlainText
            text: chip.modelData.amount
            color: chips.host.foreground
            font.family: chips.fontFamily
            font.pixelSize: Style.font.bodySmall
          }

          Text {
            textFormat: Text.PlainText
            text: "✓︎"
            visible: chip.modelData.met
            color: chips.tint
            font.family: chips.fontFamily
            font.pixelSize: Style.font.bodySmall
          }
        }

        Rectangle {
          id: bar
          anchors.left: parent.left
          anchors.right: parent.right
          anchors.bottom: parent.bottom
          anchors.leftMargin: Style.space(6)
          anchors.rightMargin: Style.space(6)
          anchors.bottomMargin: Style.space(3)
          height: Style.space(2)
          radius: height / 2
          color: Qt.rgba(chips.host.foreground.r, chips.host.foreground.g, chips.host.foreground.b, 0.14)

          Rectangle {
            width: parent.width * chip.modelData.fraction
            height: parent.height
            radius: parent.radius
            color: chip.modelData.met ? chips.tint : chips.host.mutedForeground
          }
        }
      }
    }

    Text {
      visible: chips.summary.more > 0
      height: Style.space(22)
      verticalAlignment: Text.AlignVCenter
      textFormat: Text.PlainText
      text: chips.host.tr.t("goal.more").arg(chips.summary.more)
      color: chips.host.mutedForeground
      font.family: chips.fontFamily
      font.pixelSize: Style.font.bodySmall
    }
  }
}
