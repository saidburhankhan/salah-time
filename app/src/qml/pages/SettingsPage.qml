import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
Item {
 id: page; property var win; property string section: "project"
 Component.onCompleted: backend.loadSettings()
 ColumnLayout { anchors.fill: parent; anchors.margins: 28; spacing: 14
  RowLayout { Repeater { model: [["project",qsTr("Projects")],["tag",qsTr("Tags")],["metric",qsTr("Metrics")]]; AppButton { required property var modelData; win: page.win; text: modelData[1]; primary: page.section===modelData[0]; onClicked: page.section=modelData[0] } } Item { Layout.fillWidth: true } }
  Card { win: page.win; Layout.fillWidth: true; height: 92
   RowLayout { anchors.fill: parent; anchors.margins: 16
    TextField { id: name; Layout.fillWidth: true; placeholderText: page.section==="project"?qsTr("Project name"):page.section==="tag"?qsTr("Tag name"):qsTr("Metric name") }
    TextField { id: color; visible: page.section!=="metric"; text: "#7c9a6d"; Layout.preferredWidth: 110 }
    TextField { id: unit; visible: page.section==="metric"; placeholderText: qsTr("Unit"); Layout.preferredWidth: 100 }
    ComboBox { id: kind; visible: page.section==="metric"; model: ["count","scale","check"] }
    AppButton { win: page.win; text: qsTr("Add"); primary: true; onClicked: { var f={name:name.text}; if(page.section==="metric"){f.unit=unit.text;f.kind=kind.currentText;if(f.kind==="scale")f.scale_max=5}else f.color=color.text; backend.saveEntity(page.section,f);name.clear() } }
   }
  }
  ListView { Layout.fillWidth: true; Layout.fillHeight: true; spacing: 8; clip: true; model: page.section==="project"?backend.projects:page.section==="tag"?backend.tags:backend.metrics
   delegate: Card { required property var modelData; win: page.win; width: ListView.view.width; height: 72
    RowLayout { anchors.fill: parent; anchors.margins: 14
     Rectangle { visible: modelData.color!==undefined; width: 12;height:12;radius:6;color:String(modelData.color||"transparent") }
     Label { text:String(modelData.name||"");textFormat:Text.PlainText;color:page.win.text;Layout.fillWidth:true }
     Label { visible:page.section==="project";text:qsTr("%1 tags · %2 metrics").arg((modelData.tags||[]).length).arg((modelData.metrics||[]).length);color:page.win.muted }
     Label { visible:page.section==="metric";text:String(modelData.kind||"")+(modelData.builtin?qsTr(" · built-in"):"");textFormat:Text.PlainText;color:page.win.muted }
     AppButton { visible:page.section!=="tag"&&!(page.section==="metric"&&modelData.builtin);win:page.win;text:modelData.archived?qsTr("Restore"):qsTr("Archive");onClicked:backend.saveEntity(page.section,{id:modelData.id,archived:!modelData.archived}) }
    }
   }
  }
 }
}
