import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../controls"
Item {
 id: page; property var win; property var tip; property int range: 28; property var stats: backend.statsData
 Component.onCompleted: backend.loadStats(range)
 Flickable { anchors.fill: parent; contentWidth: width; contentHeight: body.implicitHeight+56; clip:true
  ColumnLayout { id:body;width:parent.width-56;x:28;y:24;spacing:14
   RowLayout { Label{text:qsTr("Filter");color:page.win.muted} ComboBox{model:[qsTr("All")].concat((backend.tags||[]).map(function(x){return String(x.name)})).concat((backend.projects||[]).map(function(x){return String(x.name)}))} Item{Layout.fillWidth:true} Repeater{model:[7,28,90];AppButton{required property var modelData;win:page.win;text:String(modelData);primary:page.range===modelData;onClicked:{page.range=modelData;backend.loadStats(modelData)}}} }
   GridLayout { columns:4;Layout.fillWidth:true;Repeater { model:[[qsTr("Minutes"),(page.stats.totals||{}).minutes||0],[qsTr("Active days"),(page.stats.totals||{}).active_days||0],[qsTr("Avg focus"),(page.stats.totals||{}).avg_focus||"—"],[qsTr("Fulfilment"),Math.round(100*((page.stats.days||[]).reduce(function(a,d){return a+Number(d.fulfilment_ratio||0)},0)/Math.max(1,(page.stats.days||[]).length)))+"%"]];Card{required property var modelData;win:page.win;Layout.fillWidth:true;height:86;Column{anchors.centerIn:parent;spacing:5;Label{text:String(modelData[1]);textFormat:Text.PlainText;color:page.win.text;font.pixelSize:22;font.bold:true}Label{text:String(modelData[0]);textFormat:Text.PlainText;color:page.win.muted}}} } }
   Label{text:qsTr("Minutes per day vs capacity");color:page.win.text;font.pixelSize:17;font.bold:true}
   Row { Layout.fillWidth:true;height:170;spacing:4;Repeater{model:page.stats.days||[];Item{required property var modelData;width:Math.max(8,(body.width-20)/Math.max(1,(page.stats.days||[]).length));height:parent.height;Rectangle{anchors.bottom:parent.bottom;width:parent.width;height:parent.height*Math.min(1,Number(modelData.capacity_minutes||0)/1440);color:"#34312b"}Rectangle{anchors.bottom:parent.bottom;width:parent.width;height:parent.height*Math.min(1,Number(modelData.minutes||0)/Math.max(1,Number(modelData.capacity_minutes||1)));color:theme.accent}}} }
   Label{text:qsTr("Metric series");color:page.win.text;font.pixelSize:17;font.bold:true}
   Repeater{model:page.stats.series||[];Card{required property var modelData;win:page.win;Layout.fillWidth:true;height:70;RowLayout{anchors.fill:parent;anchors.margins:14;Label{text:String(modelData.name||"");textFormat:Text.PlainText;color:page.win.text;Layout.fillWidth:true}Label{text:String((modelData.points||[]).length?modelData.points[modelData.points.length-1].value:"—")+" "+String(modelData.unit||"");textFormat:Text.PlainText;color:page.win.muted}}}}
   Label{text:qsTr("Projects");color:page.win.text;font.pixelSize:17;font.bold:true}
   Repeater{model:page.stats.projects||[];Card{required property var modelData;win:page.win;Layout.fillWidth:true;height:58;RowLayout{anchors.fill:parent;anchors.margins:12;Rectangle{width:10;height:10;radius:5;color:String(modelData.color||"transparent")}Label{text:String(modelData.name||"");textFormat:Text.PlainText;color:page.win.text;Layout.fillWidth:true}Label{text:qsTr("%1 min · %2 sessions").arg(modelData.minutes||0).arg(modelData.sessions||0);color:page.win.muted}}}}
   Label{text:qsTr("Blocks");color:page.win.text;font.pixelSize:17;font.bold:true}
   Repeater{model:page.stats.blocks||[];Card{required property var modelData;win:page.win;Layout.fillWidth:true;height:58;RowLayout{anchors.fill:parent;anchors.margins:12;Label{text:String(modelData.block||"");textFormat:Text.PlainText;color:page.win.text;Layout.fillWidth:true}Label{text:qsTr("%1 / %2 min · focus %3").arg(modelData.minutes||0).arg(modelData.capacity_minutes||0).arg(modelData.avg_focus===null?"—":modelData.avg_focus);color:page.win.muted}}}}
  }
 }
}
