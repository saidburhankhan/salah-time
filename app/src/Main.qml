import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Effects
import QtQuick.Layouts
import QtQuick.Shapes

ApplicationWindow {
    id: win
    width: 1440; height: 900; minimumWidth: 960; minimumHeight: 640
    visible: true; title: qsTr("Salah"); color: "#11110f"
    Material.theme: Material.Dark; Material.accent: theme.accent; Material.roundedScale: theme.radius
    property int page: 0
    property bool quickVisible: false
    property bool entryVisible: false
    property bool helpVisible: false
    property string toastText: ""
    property double timerNow: Date.now()
    property string quickBlock: ""
    property var editItem: ({})
    property string statsMode: "week"
    property date statsAnchor: new Date()
    property date customFrom: new Date()
    property date customTo: new Date()
    property bool customRangeVisible: false
    property bool statsTableVisible: false
    property string selectedStatsItem: ""
    property int countdown: backend.dayPlan.now && backend.dayPlan.now.next ? backend.dayPlan.now.next.remaining_seconds : 0
    property var prayerIds: ["fajr", "dhuhr", "asr", "maghrib", "isha"]
    property var blockIds: ["night", "fajr", "dhuhr", "asr", "maghrib", "isha"]
    property var blockNames: [qsTr("Before Fajr"), qsTr("Fajr"), qsTr("Dhuhr"), qsTr("Asr"), qsTr("Maghrib"), qsTr("Isha")]
    readonly property color panel: "#1b1a17"
    readonly property color card: "#201f1b"
    readonly property color border: "#34312a"
    readonly property color muted: "#8f8a80"
    readonly property color text: "#f1efe9"
    readonly property string mono: "monospace"

    function openQuick(block) { quickBlock=block||""; backend.parseQuick(""); quickVisible=true; quickField.text=""; quickField.forceActiveFocus() }
    function openEntry(item) { editItem=item||({}); entryVisible=true }
    function blockItems(id) {
        var out=[]; var blocks=backend.dayPlan.blocks||[]
        for(var i=0;i<blocks.length;i++) if(blocks[i].start===id) return blocks[i].items||[]
        return out
    }
    function locationName() { var c=backend.config||{}; return c.location_name||c.name||(typeof c.location==="string"?c.location:(c.location?c.location.name:""))||qsTr("Location") }
    function methodName() { var c=backend.config||{}; return c.method_name||c.method||"—" }
    function capitalized(s) { s=String(s||""); return s.charAt(0).toUpperCase()+s.slice(1) }
    function weekdayText(days, repeat) {
        if(repeat==="daily") return qsTr("Daily")
        var a=days||[], key=a.join(",")
        if(key==="mon,tue,wed,thu,fri") return qsTr("Mon–Fri")
        if(key==="mon,tue,wed,thu,fri,sat,sun") return qsTr("Daily")
        var names={mon:qsTr("Mon"),tue:qsTr("Tue"),wed:qsTr("Wed"),thu:qsTr("Thu"),fri:qsTr("Fri"),sat:qsTr("Sat"),sun:qsTr("Sun")}
        return a.map(function(d){return names[d]||capitalized(d)}).join(", ")
    }
    function detail(x) {
        var value=x.measure==="minutes"?qsTr("%1 / %2 min").arg(runningMinutes(x)).arg(x.target||0):""
        var repeat=x.repeat==="weekly"?weekdayText(x.weekdays,x.repeat):x.repeat==="daily"?qsTr("Daily"):qsTr("Once")
        return value ? value+"  ·  "+repeat : repeat
    }
    function runningMinutes(x) {
        var total=Number(x.minutes||0), started=String(x.timer_started_at||"")
        if(started) { var ms=Date.parse(started); if(!isNaN(ms)) total+=Math.max(0,Math.floor((timerNow-ms)/60000)) }
        return total
    }
    function weekdayId(d) { return ["sun","mon","tue","wed","thu","fri","sat"][d.getDay()] }
    function nextOccurrence(item, fromIso) {
        var d=Date.fromLocaleString(Qt.locale(),fromIso,"yyyy-MM-dd"), repeat=item.repeat||"none", days=item.weekdays||[]
        if(repeat==="none"||repeat==="daily") return d
        for(var i=0;i<14;i++){if(days.indexOf(weekdayId(d))>=0)return d;d=new Date(d.getFullYear(),d.getMonth(),d.getDate()+1)}
        return d
    }
    function occursOn(item, dayIso) {
        if((item.repeat||"none")==="none"||(item.repeat||"none")==="daily") return true
        var d=Date.fromLocaleString(Qt.locale(),dayIso,"yyyy-MM-dd")
        return (item.weekdays||[]).indexOf(weekdayId(d))>=0
    }
    function shortDate(d) { return Qt.locale("en_US").toString(d,"ddd d MMM") }
    function maybeToast(item) {
        if(occursOn(item,backend.selectedDay))return
        var start=Date.fromLocaleString(Qt.locale(),backend.selectedDay,"yyyy-MM-dd");start=new Date(start.getFullYear(),start.getMonth(),start.getDate()+1)
        toastText=String(item.title||qsTr("Item"))+qsTr(" — next %1").arg(shortDate(nextOccurrence(item,iso(start))));toastTimer.restart()
    }
    function closeTopOverlay() {
        if(entryVisible&&entryPopup.confirming){entryPopup.confirming=false;return true}
        if(helpVisible){helpVisible=false;return true}
        if(entryVisible){entryVisible=false;return true}
        if(quickVisible){quickVisible=false;return true}
        if(dragging){finishDrag(true);return true}
        return false
    }
    function joinedDetail(block, schedule) {
        var parts=[]
        if(block) parts.push(block==="night"?qsTr("Before Fajr"):capitalized(block))
        if(schedule) parts.push(schedule)
        return parts.join("  ·  ")
    }
    function calendarDate(offset) {
        var base=Date.fromLocaleString(Qt.locale(),backend.selectedDay||backend.todayDay,"yyyy-MM-dd")
        var first=new Date(base.getFullYear(),base.getMonth(),1); var monday=(first.getDay()+6)%7
        return new Date(first.getFullYear(),first.getMonth(),1-monday+offset)
    }
    function iso(d) { return d.getFullYear()+"-"+String(d.getMonth()+1).padStart(2,"0")+"-"+String(d.getDate()).padStart(2,"0") }
    function statsBounds(mode, anchor, from, to) {
        var a=new Date(anchor.getFullYear(),anchor.getMonth(),anchor.getDate()), first, last
        if(mode==="month") { first=new Date(a.getFullYear(),a.getMonth(),1);last=new Date(a.getFullYear(),a.getMonth()+1,0) }
        else if(mode==="custom") { first=new Date(from.getFullYear(),from.getMonth(),from.getDate());last=new Date(to.getFullYear(),to.getMonth(),to.getDate());if(first>last){var swap=first;first=last;last=swap} }
        else { var monday=(a.getDay()+6)%7;first=new Date(a.getFullYear(),a.getMonth(),a.getDate()-monday);last=new Date(first.getFullYear(),first.getMonth(),first.getDate()+6) }
        return {from:iso(first),to:iso(last),first:first,last:last}
    }
    function stepStats(amount) { var b=statsBounds(statsMode,statsAnchor,customFrom,customTo);if(statsMode==="custom"){var n=Math.round((b.last-b.first)/86400000)+1;customFrom=new Date(b.first.getFullYear(),b.first.getMonth(),b.first.getDate()+amount*n);customTo=new Date(b.last.getFullYear(),b.last.getMonth(),b.last.getDate()+amount*n);statsAnchor=customFrom}else if(statsMode==="month")statsAnchor=new Date(statsAnchor.getFullYear(),statsAnchor.getMonth()+amount,1);else statsAnchor=new Date(statsAnchor.getFullYear(),statsAnchor.getMonth(),statsAnchor.getDate()+amount*7);loadStatistics() }
    function loadStatistics() { var b=statsBounds(statsMode,statsAnchor,customFrom,customTo);backend.loadStatsRange(b.from,b.to) }
    function statsRangeLabel() { var b=statsBounds(statsMode,statsAnchor,customFrom,customTo),l=Qt.locale("en_US");if(statsMode==="month")return l.toString(b.first,"MMMM yyyy");if(b.first.getMonth()===b.last.getMonth())return l.toString(b.first,"d")+"–"+l.toString(b.last,"d MMM yyyy");return l.toString(b.first,"d MMM")+" – "+l.toString(b.last,"d MMM yyyy") }
    function tooltipText(day) { var d=Date.fromLocaleString(Qt.locale(),day.day,"yyyy-MM-dd");return Qt.locale("en_US").toString(d,"ddd d MMM")+" · "+(day.done||0)+" / "+(day.planned||0)+" done · "+(day.minutes||0)+" min · "+(day.count||0)+" counts · "+(day.prayers||0)+" prayers" }
    function formatMinutes(n) { n=Number(n||0);return n>=60?Math.floor(n/60)+" h "+(n%60)+" m":n+" m" }
    function selectedDaily() { if(!selectedStatsItem)return backend.stats.daily||[];var items=backend.stats.items||[];for(var i=0;i<items.length;i++)if(items[i].id===selectedStatsItem){var out=[],s=items[i].series||[];for(var j=0;j<s.length;j++)out.push({day:s[j].day,planned:s[j].planned?1:0,done:s[j].done?1:0,minutes:s[j].minutes||0,count:s[j].count||0,prayers:0});return out}return [] }
    function newestDaily() { var source=selectedDaily(),out=[];for(var i=source.length-1;i>=0;i--)out.push(source[i]);return out }
    function maxDaily(field) { var a=selectedDaily(),m=0;for(var i=0;i<a.length;i++)m=Math.max(m,Number(a[i][field]||0));return Math.max(1,m) }
    function isTyping() { var f=win.activeFocusItem; return f && f.inputMethodHints !== undefined }
    ListModel { id: timelineModel }
    property bool timelineSyncing: false
    property int timelineRevision: -1
    function desiredTimelineRows() {
        var rows=[], blocks=backend.dayPlan.blocks||[], prayers=backend.dayPlan.prayers||[]
        for (var b=0;b<blockIds.length;b++) {
            var id=blockIds[b], prayer=b>0?(prayers[b-1]||{}):({})
            rows.push({key:"header:"+id,kind:"header",block:id,payload:{name:blockNames[b],time:b===0?(backend.dayPlan.night_start||"--:--"):(prayer.time||"--:--"),marked:!!prayer.marked,current:id===(backend.dayPlan.now||{}).block}})
            var items=[]; for(var x=0;x<blocks.length;x++) if(blocks[x].start===id) { items=blocks[x].items||[]; break }
            for(var i=0;i<items.length;i++) rows.push({key:"item:"+items[i].id,kind:"item",block:id,payload:items[i]})
            rows.push({key:"add:"+id,kind:"add",block:id,payload:{}})
            if(id==="fajr") rows.push({key:"marker:sunrise",kind:"marker",block:id,payload:{name:qsTr("Sunrise"),time:backend.dayPlan.sunrise||"--:--"}})
            if(id==="isha") rows.push({key:"marker:sleep",kind:"marker",block:id,payload:{name:qsTr("Sleep"),time:(backend.dayPlan.sleep||{}).time||"--:--"}})
        }
        return rows
    }
    function syncTimeline() {
        if(dragging||timelineRevision===backend.dayRevision) return
        timelineRevision=backend.dayRevision
        timelineSyncing=true; var wanted=desiredTimelineRows()
        for(var i=0;i<wanted.length;i++) {
            var found=-1; for(var j=i;j<timelineModel.count;j++) if(timelineModel.get(j).key===wanted[i].key){found=j;break}
            if(found<0) timelineModel.insert(i,wanted[i]); else { if(found!==i) timelineModel.move(found,i,1); timelineModel.set(i,wanted[i]) }
        }
        while(timelineModel.count>wanted.length) timelineModel.remove(timelineModel.count-1)
        timelineSyncing=false
    }
    property bool dragging: false
    property string dragId: ""
    property int dragOriginalIndex: -1
    property real dragPointerY: 0
    property var dragPayload: ({})
    function beginDrag(rowIndex, id, globalY) { dragging=true;dragId=id;dragOriginalIndex=rowIndex;dragPointerY=globalY;dragPayload=timelineModel.get(rowIndex).payload||({}) }
    function updateDrag(globalY) {
        if(!dragging)return; dragPointerY=globalY
        var p=todayList.mapFromItem(win.contentItem,0,globalY), idx=todayList.indexAt(10,p.y+todayList.contentY)
        if(idx<0) idx=p.y<0?0:timelineModel.count-1
        var from=-1;for(var i=0;i<timelineModel.count;i++)if(timelineModel.get(i).key==="item:"+dragId){from=i;break}
        // The gap belongs between a block's header and its "+ add" row: over a
        // header it goes just below it, over "+ add" just above it, and markers
        // (Sunrise, Sleep) never take it. Moving down, ListModel.move puts the
        // row after the target row; moving up, before it.
        var k=timelineModel.get(idx).kind
        if(k==="marker") return
        if(k==="header"&&from>idx) idx++
        if(k==="add"&&from<idx) idx--
        if(from>=0&&idx!==from) timelineModel.move(from,Math.max(0,Math.min(idx,timelineModel.count-1)),1)
    }
    function finishDrag(cancelled) {
        if(!dragging)return
        var from=-1;for(var i=0;i<timelineModel.count;i++)if(timelineModel.get(i).key==="item:"+dragId){from=i;break}
        if(cancelled||from<0){if(from>=0&&dragOriginalIndex>=0)timelineModel.move(from,dragOriginalIndex,1)} else {
            // ListModel rows reach C++ as empty maps, so pass plain objects.
            var rows=[];for(var r=0;r<timelineModel.count;r++)if(r!==from){var e=timelineModel.get(r);rows.push({kind:e.kind,block:e.block})}
            var destination=backend.moveDestination(rows,from); if(destination.block) backend.moveItem(dragId,destination.block,destination.index)
        }
        dragging=false;dragId="";dragOriginalIndex=-1;syncTimeline()
    }
    Rectangle { visible:dragging; z:1000; x:248; width:Math.max(360,win.width-248-375); height:72; y:Math.max(76,Math.min(win.height-height-8,dragPointerY-height/2)); radius:theme.radius; color:"#292722"; border.color:"#70695d"; scale:1.02
        RowLayout { anchors.fill:parent; anchors.leftMargin:14; anchors.rightMargin:16; spacing:12
            Label { text:"⠿";color:win.muted;font.pixelSize:19 }
            Rectangle { width:24;height:24;radius:Math.min(5,theme.radius);color:dragPayload.state==="done"?theme.accent:"transparent";border.color:"#777168" }
            ColumnLayout { Layout.fillWidth:true;spacing:4
                Label { text:dragPayload.title||"";color:win.text;font.pixelSize:15;font.weight:Font.Medium;Layout.fillWidth:true;elide:Text.ElideRight;textFormat:Text.PlainText }
                Label { text:detail(dragPayload);color:win.muted;font.family:win.mono;font.pixelSize:12 }
            }
        }
    }

    component AppButton: Button {
        property bool primary: false
        implicitHeight: 42; leftPadding: 17; rightPadding: 17
        contentItem: Text { text:parent.text; color:parent.enabled?(parent.primary?"#171612":win.text):win.muted; font.pixelSize:14; horizontalAlignment:Text.AlignHCenter; verticalAlignment:Text.AlignVCenter }
        background: Rectangle { radius:Math.max(6,theme.radius); color:parent.enabled&&parent.primary?win.text:(parent.down?win.border:"transparent"); border.width:parent.primary?0:1; border.color:win.border }
    }
    component ChoiceChip: Button {
        property bool selected: false
        implicitHeight: 40; leftPadding:15; rightPadding:15
        contentItem: Text { text:parent.text; color:parent.selected?"#171612":"#d6d2ca"; font.pixelSize:14; horizontalAlignment:Text.AlignHCenter; verticalAlignment:Text.AlignVCenter }
        background: Rectangle { radius:Math.max(6,theme.radius); color:parent.selected?win.text:(parent.down?win.border:"transparent"); border.width:1; border.color:parent.selected?win.text:win.border }
    }
    component SidebarIcon: Item {
        id: sidebarIcon
        property string kind: ""
        property color iconColor: win.muted
        width:22; height:22; implicitWidth:22; implicitHeight:22
        Layout.preferredWidth:22; Layout.preferredHeight:22
        Shape {
            anchors.centerIn: parent; width:18; height:18
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: sidebarIcon.iconColor; strokeWidth:1.5; fillColor:"transparent"
                capStyle:ShapePath.RoundCap; joinStyle:ShapePath.RoundJoin
                PathSvg { path: sidebarIcon.kind === "today" ? "M3 5.5 L15 5.5 M6 2.5 L6 5 M12 2.5 L12 5 M4.5 3.5 L13.5 3.5 Q15 3.5 15 5 L15 14 Q15 15.5 13.5 15.5 L4.5 15.5 Q3 15.5 3 14 L3 5 Q3 3.5 4.5 3.5" : sidebarIcon.kind === "routines" ? "M14.5 5.5 L14.5 2.5 M14.5 5.5 L11.5 5.5 M14.3 5.3 A6.5 6.5 0 1 0 15 11" : sidebarIcon.kind === "statistics" ? "M3.5 15 L3.5 10 L6.5 10 L6.5 15 Z M7.5 15 L7.5 6.5 L10.5 6.5 L10.5 15 Z M11.5 15 L11.5 3 L14.5 3 L14.5 15 Z" : sidebarIcon.kind === "settings" ? "M3 5 L15 5 M3 9 L15 9 M3 13 L15 13 M6 3 L6 7 M12 7 L12 11 M8 11 L8 15" : "M9 16 C9 16 14 11.5 14 7.5 A5 5 0 1 0 4 7.5 C4 11.5 9 16 9 16 Z M9 9.5 A2 2 0 1 0 9 5.5 A2 2 0 1 0 9 9.5" }
            }
        }
    }

    Shortcut { sequence: "Ctrl+1"; context: Qt.ApplicationShortcut; onActivated: page=0 }
    Shortcut { sequence: "Ctrl+2"; context: Qt.ApplicationShortcut; onActivated: page=1 }
    Shortcut { sequence: "Ctrl+3"; context: Qt.ApplicationShortcut; onActivated: {page=2;loadStatistics()} }
    Shortcut { sequence: "Ctrl+4"; context: Qt.ApplicationShortcut; onActivated: page=3 }
    Shortcut { sequence: "Ctrl+K"; context: Qt.ApplicationShortcut; onActivated: openQuick("") }
    Shortcut { sequence: "Left"; context: Qt.ApplicationShortcut; enabled: page===0&&!quickVisible&&!entryVisible; onActivated: backend.moveDay(-1) }
    Shortcut { sequence: "Right"; context: Qt.ApplicationShortcut; enabled: page===0&&!quickVisible&&!entryVisible; onActivated: backend.moveDay(1) }
    Shortcut { sequence: "Left"; context: Qt.ApplicationShortcut; enabled: page===2&&!customRangeVisible&&!isTyping(); onActivated: stepStats(-1) }
    Shortcut { sequence: "Right"; context: Qt.ApplicationShortcut; enabled: page===2&&!customRangeVisible&&!isTyping(); onActivated: stepStats(1) }
    Shortcut { sequence: "T"; context: Qt.ApplicationShortcut; enabled: !isTyping(); onActivated: backend.jumpToday() }
    Shortcut { sequence: "?"; context: Qt.ApplicationShortcut; onActivated: helpVisible=!helpVisible }
    Shortcut { sequence: "Q"; context: Qt.ApplicationShortcut; enabled: !isTyping(); onActivated: Qt.quit() }
    Shortcut { sequence: "Ctrl+Q"; context: Qt.ApplicationShortcut; onActivated: Qt.quit() }
    Shortcut { sequence: "Ctrl+W"; context: Qt.ApplicationShortcut; onActivated: Qt.quit() }
    Timer { interval:1000; running:true; repeat:true; onTriggered:{if(countdown>0)countdown--;timerNow=Date.now()} }
    Timer { interval:60000; running:true; repeat:true; onTriggered: backend.reload() }
    Connections { target:backend; function onChanged(){ if(backend.dayPlan.now&&backend.dayPlan.now.next) countdown=backend.dayPlan.now.next.remaining_seconds||0; syncTimeline() } }
    Shortcut { sequence:"Escape"; context:Qt.ApplicationShortcut; onActivated:{if(customRangeVisible)customRangeVisible=false;else closeTopOverlay()} }
    onActiveChanged: if(active) backend.reload()

    RowLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.preferredWidth: 220; Layout.fillHeight: true; color: "#1c1b18"
            border.color: win.border; border.width: 0
            ColumnLayout { anchors.fill:parent; anchors.margins:18; spacing:8
                RowLayout { Layout.fillWidth:true; Layout.preferredHeight:64; spacing:12
                    Item { width:28;height:28
                        Rectangle { width:24;height:24;radius:12;color:win.text;anchors.centerIn:parent }
                        Rectangle { width:20;height:20;radius:10;color:"#1c1b18";x:10;y:0 }
                    }
                    Label { text:"Salah"; color:win.text; font.pixelSize:21; font.weight:Font.DemiBold }
                }
                Repeater { model:[qsTr("Today"),qsTr("Routines"),qsTr("Statistics")]
                    Rectangle { required property string modelData; required property int index; Layout.fillWidth:true; height:48; radius:theme.radius
                        color:win.page===index?win.card:"transparent"; border.color:win.page===index?win.border:"transparent"
                        Row { anchors.verticalCenter:parent.verticalCenter; leftPadding:14; spacing:13
                            SidebarIcon { kind:index===0?"today":index===1?"routines":"statistics"; iconColor:win.page===index?win.text:win.muted }
                            Label { text:modelData; color:win.page===index?win.text:"#c4c0b8"; font.pixelSize:16 }
                        }
                        MouseArea { anchors.fill:parent; onClicked:{win.page=index;if(index===2)loadStatistics()} }
                    }
                }
                Rectangle { Layout.fillWidth:true; height:1; color:win.border; Layout.topMargin:8; Layout.bottomMargin:8 }
                Rectangle { Layout.fillWidth:true; height:48; radius:theme.radius; color:win.page===3?win.card:"transparent"; border.color:win.page===3?win.border:"transparent"
                    Row { anchors.verticalCenter:parent.verticalCenter; leftPadding:14; spacing:13
                        SidebarIcon { kind:"settings"; iconColor:win.page===3?win.text:win.muted }
                        Label { text:qsTr("Settings"); color:win.page===3?win.text:"#c4c0b8"; font.pixelSize:16 }
                    }
                    MouseArea { anchors.fill:parent; onClicked:win.page=3 }
                }
                Item { Layout.fillHeight:true }
                RowLayout { Layout.fillWidth:true; Layout.preferredHeight:48
                    SidebarIcon { kind:"location"; iconColor:win.muted; Layout.leftMargin:12 }
                    Label { text:locationName(); color:"#c4c0b8"; font.pixelSize:15; Layout.fillWidth:true; elide:Text.ElideRight }
                }
            }
            Rectangle { anchors.right:parent.right; width:1; height:parent.height; color:win.border }
        }

        Item {
            Layout.fillWidth:true; Layout.fillHeight:true
            ColumnLayout { anchors.fill:parent; spacing:0
                Rectangle { Layout.fillWidth:true; height:74; color:"#171612"
                    RowLayout { anchors.fill:parent; anchors.leftMargin:28; anchors.rightMargin:28
                        ColumnLayout { visible:page===0; Layout.fillWidth:true; spacing:2
                            RowLayout { spacing:10
                                Label { text:(backend.selectedDay===backend.todayDay?qsTr("Today · "):"")+backend.hijriHeader(backend.selectedDay); color:win.text; font.pixelSize:22; font.weight:Font.DemiBold }
                                Label { text:backend.gregorianHeader(backend.selectedDay); color:win.muted; font.family:win.mono; font.pixelSize:15 }
                            }
                        }
                        Label { visible:page!==0; Layout.fillWidth:true; text:page===1?qsTr("Routines"):page===2?qsTr("Statistics"):qsTr("Settings"); color:win.text; font.pixelSize:22; font.weight:Font.DemiBold }
                        AppButton { visible:page===0; text:qsTr("+  Add"); primary:true; Layout.alignment:Qt.AlignRight; onClicked:openQuick("") }
                    }
                    Rectangle { anchors.bottom:parent.bottom; height:1; width:parent.width; color:win.border }
                }
                Rectangle { visible:backend.error!==""; Layout.fillWidth:true; height:44; color:"#4a2724"
                    RowLayout { anchors.fill:parent; anchors.leftMargin:24; anchors.rightMargin:18
                        Label { Layout.fillWidth:true; text:backend.error; color:"#ffd5cf"; textFormat:Text.PlainText; elide:Text.ElideRight }
                        ToolButton { text:"×"; onClicked:backend.dismissError() }
                    }
                }

                // Today
                RowLayout { visible:page===0; Layout.fillWidth:true; Layout.fillHeight:true; spacing:26; Layout.margins:28
                    ListView { id:todayList; Layout.fillWidth:true; Layout.fillHeight:true; Layout.minimumWidth:360; clip:true; spacing:8; model:timelineModel
                        move: Transition { NumberAnimation { properties:"y"; duration:120; easing.type:Easing.OutCubic } }
                        displaced: Transition { NumberAnimation { properties:"y"; duration:120; easing.type:Easing.OutCubic } }
                        delegate: Item { id:rowDelegate; required property int index; required property string kind; required property string block; required property var payload; required property string key
                            width:todayList.width; height:kind==="header"?60:kind==="item"?82:kind==="add"?32:26
                            property string correctionError: ""
                            Rectangle { visible:kind==="header"; anchors.fill:parent; radius:theme.radius; color:win.card; border.color:win.border
                                        RowLayout { anchors.fill:parent; anchors.leftMargin:18; anchors.rightMargin:16; spacing:14
                                            Rectangle { visible:block!=="night"; width:28;height:28;radius:14;color:payload.marked?theme.accent:"transparent";border.width:2;border.color:payload.marked?theme.accent:win.text
                                                Label { anchors.centerIn:parent; visible:!!payload.marked; text:"✓"; color:theme.accentForeground; font.weight:Font.Bold }
                                                MouseArea { anchors.fill:parent; onClicked:backend.togglePrayer(block,payload.marked||false) }
                                            }
                                            Label { text:payload.name||""; color:win.text; font.pixelSize:17; font.weight:Font.Medium; Layout.fillWidth:true }
                                            Label { text:payload.time||"--:--"; color:"#d8d4cc"; font.family:win.mono; font.pixelSize:17 }
                                            Rectangle { visible:payload.current||false; width:62;height:28;radius:14;color:"#4a2d26"
                                                Label { anchors.centerIn:parent;text:qsTr("● NOW");color:"#ef8068";font.family:win.mono;font.pixelSize:11 }
                                            }
                                        }
                            }
                            Rectangle { id:itemCard; visible:kind==="item"; anchors.fill:parent; radius:theme.radius; color:win.dragging&&win.dragId===payload.id?"transparent":"#1c1b18"; border.color:win.dragging&&win.dragId===payload.id?"#4a463e":win.border; opacity:payload.pending?0.55:1
                                            RowLayout { anchors.fill:parent; anchors.leftMargin:12; anchors.rightMargin:14; spacing:12; opacity:win.dragging&&win.dragId===payload.id?0:1
                                                Label { id:handle; text:"⠿"; color:win.muted; font.pixelSize:19
                                                    MouseArea { anchors.fill:parent; anchors.margins:-8; preventStealing:true
                                                        onPressed:function(mouse){var p=mapToItem(win.contentItem,mouse.x,mouse.y);win.beginDrag(rowDelegate.index,payload.id,p.y)}
                                                        onPositionChanged:function(mouse){if(pressed){var p=mapToItem(win.contentItem,mouse.x,mouse.y);win.updateDrag(p.y)}}
                                                        onReleased:function(mouse){var p=mapToItem(todayList,mouse.x,mouse.y);win.finishDrag(p.y<0||p.y>todayList.height)}
                                                        onCanceled:win.finishDrag(true)
                                                    }
                                                }
                                                Rectangle { width:24;height:24;radius:Math.min(5,theme.radius);color:payload.state==="done"?theme.accent:"transparent";border.color:payload.state==="done"?theme.accent:"#625e55"
                                                    Label { anchors.centerIn:parent;text:"✓";visible:payload.state==="done";color:theme.accentForeground }
                                                    MouseArea { anchors.fill:parent;onClicked:backend.toggleDone(payload.id,payload.state) }
                                                }
                                                ColumnLayout { Layout.fillWidth:true;spacing:4
                                                    Label { text:payload.title||"";color:payload.state==="done"?win.muted:win.text;font.pixelSize:15;font.weight:Font.Medium;Layout.fillWidth:true;elide:Text.ElideRight;textFormat:Text.PlainText }
                                                    Label { text:detail(payload);color:win.muted;font.family:win.mono;font.pixelSize:12 }
                                                }
                                                RowLayout { visible:payload.measure==="count";spacing:3
                                                    RoundButton { text:"−";flat:true;Layout.preferredWidth:30;Layout.preferredHeight:30;enabled:(payload.count||0)>0;onClicked:backend.adjustCount(payload.id,-1) }
                                                    Label { text:payload.target?qsTr("%1 / %2").arg(payload.count||0).arg(payload.target):String(payload.count||0);color:win.text;font.family:win.mono;font.pixelSize:14;horizontalAlignment:Text.AlignHCenter;Layout.minimumWidth:payload.target?58:28 }
                                                    RoundButton { text:"+";flat:true;Layout.preferredWidth:30;Layout.preferredHeight:30;onClicked:backend.adjustCount(payload.id,1) }
                                                }
                                                Button { flat:true;text:payload.timer_started_at?qsTr("❚❚ %1m").arg(runningMinutes(payload)):qsTr("▶︎ %1m").arg(runningMinutes(payload));font.family:win.mono;onClicked:backend.toggleTimer(payload.id,!!payload.timer_started_at) }
                                                ColumnLayout { spacing:1
                                                    Rectangle { Layout.preferredWidth:42;Layout.preferredHeight:36;radius:7;color:"transparent";border.color:correctionError?"#ef8068":win.border
                                                        TextField { id:correctionInput;anchors.fill:parent;anchors.margins:1;color:win.text;font.family:win.mono;font.pixelSize:14;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter;placeholderText:"±";inputMethodHints:Qt.ImhFormattedNumbersOnly;background:null;padding:2
                                                            onAccepted:{var raw=text.trim(),n=Number(raw);if(!/^[+-]?\d+$/.test(raw)||n===0||n < -1440||n > 1440){correctionError=qsTr("Use ±1–1440");correctionErrorTimer.restart()}else{backend.adjustMinutes(payload.id,n);text=""}}
                                                            Keys.onEscapePressed:function(event){text="";correctionError="";focus=false;event.accepted=true}
                                                        }
                                                    }
                                                    Label { visible:correctionError!=="";text:correctionError;color:"#ef8068";font.pixelSize:9;Layout.alignment:Qt.AlignHCenter }
                                                }
                                                Timer { id:correctionErrorTimer;interval:2200;onTriggered:correctionError="" }
                                            }
                                            MouseArea { anchors.fill:parent; acceptedButtons:Qt.LeftButton; onDoubleClicked:openEntry(payload); z:-1 }
                            }
                            Rectangle { visible:kind==="add"; anchors.fill:parent;color:"transparent"
                                        Label { anchors.left:parent.left;anchors.leftMargin:56;anchors.verticalCenter:parent.verticalCenter;text:qsTr("+  add");color:"#777269";font.pixelSize:13 }
                                        MouseArea { anchors.fill:parent;onClicked:openQuick(block) }
                            }
                            Row { visible:kind==="marker"; height:26; spacing:12; leftPadding:18
                                        Rectangle { width:8;height:8;radius:4;color:"#555148";anchors.verticalCenter:parent.verticalCenter }
                                        Label { text:(payload.name||"")+"  "+(payload.time||"--:--");color:win.muted;font.family:win.mono;font.pixelSize:12;anchors.verticalCenter:parent.verticalCenter }
                            }
                        }
                    }
                    ColumnLayout { Layout.preferredWidth:300; Layout.minimumWidth:270; Layout.maximumWidth:320; Layout.fillHeight:true; spacing:18
                        Rectangle { Layout.fillWidth:true; height:210; radius:theme.radius; color:win.panel; border.color:win.border
                            ColumnLayout { anchors.fill:parent;anchors.margins:22;spacing:8
                                RowLayout { Layout.fillWidth:true
                                    Label { text:qsTr("Now");color:win.text;font.pixelSize:18;font.weight:Font.DemiBold }
                                    Label { Layout.fillWidth:true;text:(backend.dayPlan.now||{}).block==="night"?qsTr("Before Fajr"):((backend.dayPlan.now||{}).block?capitalized((backend.dayPlan.now||{}).block):"");color:win.muted;font.family:win.mono;horizontalAlignment:Text.AlignRight }
                                }
                                Rectangle { Layout.fillWidth:true;height:1;color:win.border;Layout.topMargin:6;Layout.bottomMargin:18 }
                                Label { Layout.alignment:Qt.AlignHCenter;text:qsTr("NEXT · %1").arg(((backend.dayPlan.now||{}).next||{}).name||"—");color:win.muted;font.family:win.mono;font.pixelSize:13;font.letterSpacing:1 }
                                Label { Layout.alignment:Qt.AlignHCenter;text:Math.floor(countdown/3600)+"h "+String(Math.floor((countdown%3600)/60)).padStart(2,"0")+"m";color:win.text;font.pixelSize:34;font.weight:Font.Medium }
                                Label { Layout.alignment:Qt.AlignHCenter;text:qsTr("at %1").arg(((backend.dayPlan.now||{}).next||{}).time||"--:--");color:win.muted;font.family:win.mono;font.pixelSize:14 }
                            }
                        }
                        Rectangle { Layout.fillWidth:true; height:360; radius:theme.radius; color:win.panel; border.color:win.border
                            ColumnLayout { anchors.fill:parent;anchors.margins:18;spacing:6
                                RowLayout { Layout.fillWidth:true
                                    ToolButton { text:"‹";onClicked:{var d=calendarDate(0);backend.selectDay(iso(new Date(d.getFullYear(),d.getMonth()-1,d.getDate())))} }
                                    Label { Layout.fillWidth:true;text:Qt.locale().toString(Date.fromLocaleString(Qt.locale(),backend.selectedDay,"yyyy-MM-dd"),"MMMM yyyy");horizontalAlignment:Text.AlignHCenter;color:win.text;font.pixelSize:16;font.weight:Font.Medium }
                                    ToolButton { text:"›";onClicked:{var d=calendarDate(0);backend.selectDay(iso(new Date(d.getFullYear(),d.getMonth()+1,d.getDate())))} }
                                }
                                GridLayout { columns:7;Layout.fillWidth:true;rowSpacing:5;columnSpacing:4
                                    Repeater { model:["M","T","W","T","F","S","S"]
                                        Label { required property string modelData;Layout.fillWidth:true;text:modelData;color:win.muted;font.family:win.mono;font.pixelSize:11;horizontalAlignment:Text.AlignHCenter }
                                    }
                                    Repeater { model:42
                                        Rectangle { required property int index; property var date:calendarDate(index);property string dayIso:iso(date);Layout.fillWidth:true;height:34;radius:17;color:dayIso===backend.selectedDay?win.text:"transparent";border.width:dayIso===backend.todayDay&&dayIso!==backend.selectedDay?1:0;border.color:theme.accent
                                            Label { anchors.centerIn:parent;text:parent.date.getDate();color:parent.dayIso===backend.selectedDay?"#151411":parent.date.getMonth()===calendarDate(15).getMonth()?"#d5d1c9":"#5f5a51";font.family:win.mono;font.pixelSize:13 }
                                            MouseArea { anchors.fill:parent;onClicked:backend.selectDay(parent.dayIso) }
                                        }
                                    }
                                }
                            }
                        }
                        Item { Layout.fillHeight:true }
                    }
                }

                // Routines
                ScrollView { id:routinesScroll; visible:page===1; Layout.fillWidth:true;Layout.fillHeight:true;clip:true;Layout.margins:30
                    Column { width:routinesScroll.availableWidth;spacing:12
                        Label { text:qsTr("Your repeating rhythm");color:win.muted;font.pixelSize:14;bottomPadding:8 }
                        Repeater { model:(backend.routinesStats.items||[]).filter(function(x){return x.repeat!=="none"})
                            Rectangle { required property var modelData;width:parent.width;height:116;radius:theme.radius;color:win.panel;border.color:win.border
                                RowLayout { anchors.fill:parent;anchors.margins:18;spacing:24
                                    ColumnLayout { Layout.preferredWidth:270;spacing:5
                                        Label { text:modelData.title;color:win.text;font.pixelSize:17;font.weight:Font.Medium;Layout.fillWidth:true;elide:Text.ElideRight }
                                        Label { text:joinedDetail(modelData.block,weekdayText(modelData.weekdays,modelData.repeat));color:win.muted;font.family:win.mono;font.pixelSize:12 }
                                        Label { text:modelData.planned_days===0?qsTr("Starts %1").arg(shortDate(nextOccurrence(modelData,backend.todayDay))):qsTr("%1 of %2 days").arg(modelData.done_days).arg(modelData.planned_days);color:"#c7c2b9";font.pixelSize:13 }
                                    }
                                    RowLayout { Layout.fillWidth:true;Layout.fillHeight:true;spacing:5
                                        Repeater { model:modelData.series||[]
                                            Item { required property var modelData;Layout.fillWidth:true;Layout.fillHeight:true
                                                Rectangle { anchors.horizontalCenter:parent.horizontalCenter;anchors.bottom:parent.bottom;width:modelData.measure==="check"?7:Math.max(5,parent.width-3);height:modelData.done?Math.max(7,Math.min(parent.height-12,(modelData.minutes||modelData.count||1)*2)):5;radius:3;color:modelData.done?theme.accent:"#38352f" }
                                            }
                                        }
                                    }
                                    ColumnLayout { Layout.preferredWidth:120
                                        Label { text:qsTr("%1 min").arg(modelData.minutes||0);color:win.text;font.family:win.mono;font.pixelSize:16;Layout.alignment:Qt.AlignRight }
                                        Label { visible:modelData.measure==="count";text:qsTr("%1 count").arg(modelData.count||0);color:win.text;font.family:win.mono;font.pixelSize:14;Layout.alignment:Qt.AlignRight }
                                        Label { text:qsTr("total");color:win.muted;Layout.alignment:Qt.AlignRight }
                                    }
                                }
                                MouseArea { anchors.fill:parent;onClicked:openEntry(modelData) }
                            }
                        }
                    }
                }

                // Statistics
                ScrollView { id:statsScroll; visible:page===2;Layout.fillWidth:true;Layout.fillHeight:true;clip:true;Layout.margins:26
                    Column { width:statsScroll.availableWidth;spacing:16
                        RowLayout { width:parent.width;spacing:10
                            Rectangle { height:38;width:250;radius:8;color:win.panel;border.color:win.border
                                Row { anchors.fill:parent
                                    Repeater { model:[["week",qsTr("Week")],["month",qsTr("Month")],["custom",qsTr("Custom")]]
                                        Rectangle { required property var modelData;width:parent.parent.width/3;height:parent.parent.height;radius:7;color:statsMode===modelData[0]?"#3a3730":"transparent"
                                            Label { anchors.centerIn:parent;text:modelData[1];color:statsMode===modelData[0]?win.text:win.muted;font.pixelSize:13 }
                                            MouseArea { anchors.fill:parent;onClicked:{statsMode=modelData[0];if(statsMode==="custom")customRangeVisible=true;else loadStatistics()} }
                                        }
                                    }
                                }
                            }
                            AppButton { text:"‹";implicitWidth:40;onClicked:stepStats(-1) }
                            Label { Layout.preferredWidth:210;text:statsRangeLabel();color:win.text;font.pixelSize:15;font.weight:Font.Medium;horizontalAlignment:Text.AlignHCenter }
                            AppButton { text:"›";implicitWidth:40;onClicked:stepStats(1) }
                            Item { Layout.fillWidth:true }
                            AppButton { text:qsTr("Today");onClicked:{statsAnchor=new Date();loadStatistics()} }
                        }
                        Row { width:parent.width;spacing:12
                            Repeater { model:[
                                [qsTr("Tasks done"),(backend.stats.totals||{}).done+" / "+(backend.stats.totals||{}).planned,((backend.stats.totals||{}).planned?Math.round(100*(backend.stats.totals||{}).done/(backend.stats.totals||{}).planned):0)+"% completed"],
                                [qsTr("Time spent"),formatMinutes((backend.stats.totals||{}).minutes),qsTr("%1 active days").arg((backend.stats.totals||{}).active_days||0)],
                                [qsTr("Counts"),String((backend.stats.totals||{}).count||0),qsTr("logged in this range")],
                                [qsTr("Prayers"),(backend.stats.totals||{}).prayers+" / "+(backend.stats.prayer_slots||0),qsTr("five daily prayers")]]
                                Rectangle { required property var modelData;width:(parent.width-36)/4;height:116;radius:theme.radius;color:win.panel;border.color:win.border
                                    Column { anchors.fill:parent;anchors.margins:16;spacing:7
                                        Label { text:modelData[0];color:win.muted;font.pixelSize:13 }
                                        Label { text:modelData[1];color:win.text;font.family:win.mono;font.pixelSize:27;font.weight:Font.DemiBold }
                                        Label { text:modelData[2];color:win.muted;font.pixelSize:12 }
                                    }
                                }
                            }
                        }
                        RowLayout { width:parent.width
                            Label { text:selectedStatsItem?qsTr("Filtered task"):qsTr("Daily activity");color:win.text;font.pixelSize:18;font.weight:Font.DemiBold;Layout.fillWidth:true }
                            AppButton { visible:selectedStatsItem;text:qsTr("All tasks  ×");onClicked:selectedStatsItem="" }
                            Rectangle { width:138;height:34;radius:7;color:win.panel;border.color:win.border
                                Row { anchors.fill:parent
                                    Repeater { model:[[false,qsTr("Chart")],[true,qsTr("Table")]]
                                        Rectangle { required property var modelData;width:69;height:34;radius:6;color:statsTableVisible===modelData[0]?"#3a3730":"transparent"
                                            Label { anchors.centerIn:parent;text:modelData[1];color:statsTableVisible===modelData[0]?win.text:win.muted;font.pixelSize:12 }
                                            MouseArea { anchors.fill:parent;onClicked:statsTableVisible=modelData[0] }
                                        }
                                    }
                                }
                            }
                        }
                        Column { visible:!statsTableVisible;width:parent.width;spacing:12
                            Repeater { model:[[qsTr("Tasks done per day"),"done"],[qsTr("Time spent per day"),"minutes"]]
                                Rectangle { required property var modelData;width:parent.width;height:218;radius:theme.radius;color:win.panel;border.color:win.border
                                    property var rows:selectedDaily();property real maximum:modelData[1]==="done"?Math.max(maxDaily("planned"),maxDaily("done")):maxDaily("minutes")
                                    Label { x:18;y:15;text:modelData[0];color:win.text;font.pixelSize:15;font.weight:Font.Medium }
                                    Item { id:plot;anchors{left:parent.left;right:parent.right;top:parent.top;bottom:parent.bottom;leftMargin:52;rightMargin:18;topMargin:48;bottomMargin:34}
                                        Repeater { model:4
                                            Item { required property int index;width:plot.width;height:1;y:index*(plot.height-1)/3
                                                Rectangle { width:parent.width;height:1;color:"#302e29" }
                                                Label { x:-45;y:-7;width:38;horizontalAlignment:Text.AlignRight;color:win.muted;font.family:win.mono;font.pixelSize:10;text:{var v=Math.round(parent.parent.parent.maximum*(3-index)/3);return parent.parent.parent.modelData[1]==="minutes"&&parent.parent.parent.maximum>=120?(v/60).toFixed(v%60?1:0)+"h":v} }
                                            }
                                        }
                                        Row { anchors.fill:parent;spacing:2
                                            Repeater { model:plot.parent.rows
                                                Item { required property var modelData;required property int index;width:(plot.width-Math.max(0,plot.parent.rows.length-1)*2)/Math.max(1,plot.parent.rows.length);height:plot.height
                                                    property bool future:modelData.day>backend.todayDay
                                                    Rectangle { visible:parent.parent.parent.parent.modelData[1]==="done";anchors.bottom:parent.bottom;anchors.horizontalCenter:parent.horizontalCenter;width:Math.max(3,Math.min(42,parent.width-3));height:plot.height*(modelData.planned||0)/parent.parent.parent.parent.maximum;radius:4;color:"#45423b";opacity:future?.3:1 }
                                                    Rectangle { anchors.bottom:parent.bottom;anchors.horizontalCenter:parent.horizontalCenter;width:Math.max(3,Math.min(42,parent.width-3));height:plot.height*Number(modelData[parent.parent.parent.parent.modelData[1]]||0)/parent.parent.parent.parent.maximum;radius:4;color:theme.accent;opacity:future?.22:.9 }
                                                    Rectangle { visible:modelData.day===backend.todayDay;anchors.bottom:parent.bottom;anchors.horizontalCenter:parent.horizontalCenter;width:Math.max(5,Math.min(46,parent.width-1));height:parent.height;color:"transparent";border.color:win.text;border.width:1;opacity:.55 }
                                                    Label { anchors.top:parent.bottom;anchors.topMargin:7;anchors.horizontalCenter:parent.horizontalCenter;color:future?"#5f5b54":win.muted;font.family:win.mono;font.pixelSize:10;text:{var d=Date.fromLocaleString(Qt.locale(),modelData.day,"yyyy-MM-dd");if(statsMode==="week")return Qt.locale("en_US").toString(d,"ddd").charAt(0);return (index%Math.max(1,Math.ceil(plot.parent.rows.length/12))===0||index===plot.parent.rows.length-1)?d.getDate():""} }
                                                    MouseArea { anchors.fill:parent;anchors.leftMargin:-2;anchors.rightMargin:-2;hoverEnabled:true;onEntered:{tip.text=tooltipText(modelData);tip.visible=true;var p=mapToItem(win.contentItem,width/2,0);tip.x=Math.max(230,Math.min(win.width-tip.width-10,p.x-tip.width/2));tip.y=Math.max(10,p.y-tip.height-8)} onExited:tip.visible=false }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Rectangle { width:parent.width;height:106;radius:theme.radius;color:win.panel;border.color:win.border
                                Label { x:18;y:15;text:qsTr("Prayers per day");color:win.text;font.pixelSize:15;font.weight:Font.Medium }
                                Row { x:18;y:53;width:parent.width-36;spacing:2
                                    Repeater { model:selectedDaily()
                                        Item { required property var modelData;width:(parent.parent.width-Math.max(0,selectedDaily().length-1)*2)/Math.max(1,selectedDaily().length);height:35
                                            Row { anchors.centerIn:parent;spacing:2;Repeater { model:5;Rectangle { required property int index;width:Math.max(2,Math.min(6,(parent.parent.width-8)/5));height:width;radius:width/2;color:index<(parent.parent.modelData.prayers||0)?theme.accent:"#45423b" } } }
                                            MouseArea { anchors.fill:parent;hoverEnabled:true;onEntered:{tip.text=tooltipText(modelData);tip.visible=true;var p=mapToItem(win.contentItem,width/2,0);tip.x=Math.max(230,Math.min(win.width-tip.width-10,p.x-tip.width/2));tip.y=p.y-tip.height-8} onExited:tip.visible=false }
                                        }
                                    }
                                }
                            }
                        }
                        Rectangle { id:tableCard;visible:statsTableVisible;property var rows:newestDaily();width:parent.width;height:74+rows.length*35;radius:theme.radius;color:win.panel;border.color:win.border
                            Column { anchors.fill:parent;anchors.margins:14
                                Row { width:parent.width;height:34
                                    Repeater { model:[[qsTr("Date"),.34],[qsTr("Done / Planned"),.2],[qsTr("Time"),.16],[qsTr("Counts"),.15],[qsTr("Prayers"),.15]];Label { required property var modelData;required property int index;width:parent.width*modelData[1];text:modelData[0];color:win.muted;font.pixelSize:11;horizontalAlignment:index?Text.AlignRight:Text.AlignLeft } }
                                }
                                Repeater { model:tableCard.rows
                                    Row { required property var modelData;width:tableCard.width-28;height:35;opacity:(modelData.done||modelData.minutes||modelData.count||modelData.prayers)?.95:.38
                                        Label { width:parent.width*.34;text:Qt.locale("en_US").toString(Date.fromLocaleString(Qt.locale(),parent.modelData.day,"yyyy-MM-dd"),"ddd, d MMM");color:win.text;font.pixelSize:12 }
                                        Label { width:parent.width*.2;text:(parent.modelData.done||0)+" / "+(parent.modelData.planned||0);color:win.text;font.family:win.mono;font.pixelSize:12;horizontalAlignment:Text.AlignRight }
                                        Label { width:parent.width*.16;text:formatMinutes(parent.modelData.minutes);color:win.text;font.family:win.mono;font.pixelSize:12;horizontalAlignment:Text.AlignRight }
                                        Label { width:parent.width*.15;text:String(parent.modelData.count||0);color:win.text;font.family:win.mono;font.pixelSize:12;horizontalAlignment:Text.AlignRight }
                                        Label { width:parent.width*.15;text:(parent.modelData.prayers||0)+" / 5";color:win.text;font.family:win.mono;font.pixelSize:12;horizontalAlignment:Text.AlignRight }
                                    }
                                }
                                Row { width:parent.width;height:38;property var t:backend.stats.totals||{};Rectangle{anchors.top:parent.top;width:parent.width;height:1;color:win.border}Label{width:parent.width*.34;text:qsTr("Total");color:win.text;font.weight:Font.DemiBold}Label{width:parent.width*.2;text:(parent.t.done||0)+" / "+(parent.t.planned||0);color:win.text;font.family:win.mono;horizontalAlignment:Text.AlignRight}Label{width:parent.width*.16;text:formatMinutes(parent.t.minutes);color:win.text;font.family:win.mono;horizontalAlignment:Text.AlignRight}Label{width:parent.width*.15;text:parent.t.count||0;color:win.text;font.family:win.mono;horizontalAlignment:Text.AlignRight}Label{width:parent.width*.15;text:(parent.t.prayers||0)+" / "+(backend.stats.prayer_slots||0);color:win.text;font.family:win.mono;horizontalAlignment:Text.AlignRight} }
                            }
                        }
                        Label { text:qsTr("By task");color:win.text;font.pixelSize:18;font.weight:Font.DemiBold;topPadding:4 }
                        Repeater { model:(backend.stats.items||[]).slice().sort(function(a,b){return (b.minutes||0)-(a.minutes||0)})
                            Rectangle { required property var modelData;width:parent.width;height:88;radius:theme.radius;color:selectedStatsItem===modelData.id?"#292722":win.panel;border.color:selectedStatsItem===modelData.id?theme.accent:win.border
                                RowLayout { anchors.fill:parent;anchors.margins:15;spacing:16
                                    ColumnLayout { Layout.preferredWidth:260;spacing:4;Label{text:modelData.title;color:win.text;font.pixelSize:14;font.weight:Font.Medium;Layout.fillWidth:true;elide:Text.ElideRight}Label{text:joinedDetail(modelData.block,modelData.repeat);color:win.muted;font.family:win.mono;font.pixelSize:11} }
                                    ColumnLayout { Layout.preferredWidth:190;spacing:6;Label{text:(modelData.done_days||0)+" / "+(modelData.planned_days||0)+qsTr(" days");color:win.muted;font.family:win.mono;font.pixelSize:11}Rectangle{Layout.fillWidth:true;height:4;radius:2;color:"#403d37";Rectangle{height:parent.height;width:parent.width*(modelData.planned_days?modelData.done_days/modelData.planned_days:0);radius:2;color:theme.accent}} }
                                    Item { Layout.fillWidth:true;Layout.preferredHeight:42;Row{anchors.fill:parent;spacing:2;Repeater{model:modelData.series||[];Rectangle{required property var modelData;width:(parent.parent.width-Math.max(0,(parent.parent.parent.modelData.series||[]).length-1)*2)/Math.max(1,(parent.parent.parent.modelData.series||[]).length);height:Math.max(2,parent.parent.height*Math.min(1,(modelData.minutes||0)/60));anchors.bottom:parent.bottom;radius:2;color:"#777168"}}} }
                                    Label { Layout.preferredWidth:90;text:formatMinutes(modelData.minutes);color:win.text;font.family:win.mono;font.pixelSize:14;horizontalAlignment:Text.AlignRight }
                                    Label { Layout.preferredWidth:76;text:modelData.measure==="count"?(modelData.count||0)+" ×":"—";color:win.muted;font.family:win.mono;font.pixelSize:12;horizontalAlignment:Text.AlignRight }
                                }
                                MouseArea { anchors.fill:parent;onClicked:selectedStatsItem=modelData.id }
                            }
                        }
                        Label { visible:!backend.busy&&!(backend.stats.daily||[]).some(function(x){return x.done||x.minutes||x.count||x.prayers});width:parent.width;height:80;text:qsTr("No activity in this range");color:win.muted;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter }
                        Label { visible:backend.busy;width:parent.width;height:50;text:qsTr("Loading statistics…");color:win.muted;horizontalAlignment:Text.AlignHCenter }
                    }
                }

                // Settings
                Item { visible:page===3;Layout.fillWidth:true;Layout.fillHeight:true
                    ColumnLayout { width:620;anchors.top:parent.top;anchors.left:parent.left;anchors.margins:34;spacing:18
                        Rectangle { Layout.fillWidth:true;height:150;radius:theme.radius;color:win.panel;border.color:win.border
                            ColumnLayout { anchors.fill:parent;anchors.margins:22
                                Label { text:qsTr("Sleep time");color:win.text;font.pixelSize:17;font.weight:Font.Medium }
                                Label { text:qsTr("The end of your Isha planning block.");color:win.muted }
                                RowLayout { TextField { id:sleepField;text:backend.config.sleep_time||"23:00";placeholderText:"HH:MM";font.family:win.mono;Layout.preferredWidth:160 }
                                    AppButton { text:qsTr("Save");primary:true;onClicked:backend.setSleepTime(sleepField.text) }
                                }
                            }
                        }
                        Rectangle { Layout.fillWidth:true;height:180;radius:theme.radius;color:win.panel;border.color:win.border
                            ColumnLayout { anchors.fill:parent;anchors.margins:22;spacing:12
                                Label { text:qsTr("Prayer calculation");color:win.text;font.pixelSize:17;font.weight:Font.Medium }
                                RowLayout { Layout.fillWidth:true;Label{text:qsTr("Location");color:win.muted;Layout.fillWidth:true}Label{text:locationName();color:win.text;font.family:win.mono} }
                                RowLayout { Layout.fillWidth:true;Label{text:qsTr("Method");color:win.muted;Layout.fillWidth:true}Label{text:methodName();color:win.text;font.family:win.mono} }
                                Label { text:qsTr("Change location and calculation method with the salah CLI.");color:win.muted;font.pixelSize:12 }
                            }
                        }
                    }
                }
            }
        }
    }

    Popup { id:customRangePopup;visible:customRangeVisible;modal:true;focus:true;x:250;y:90;width:330;height:390;closePolicy:Popup.CloseOnPressOutside
        property date shownMonth:statsAnchor
        property bool pickingEnd:false
        onClosed:customRangeVisible=false
        background:Rectangle{color:win.panel;radius:theme.radius;border.color:win.border}
        Column { anchors.fill:parent;anchors.margins:18;spacing:12
            RowLayout { width:parent.width;AppButton{text:"‹";onClicked:customRangePopup.shownMonth=new Date(shownMonth.getFullYear(),shownMonth.getMonth()-1,1)}Label{Layout.fillWidth:true;text:Qt.locale("en_US").toString(customRangePopup.shownMonth,"MMMM yyyy");color:win.text;font.pixelSize:15;horizontalAlignment:Text.AlignHCenter}AppButton{text:"›";onClicked:customRangePopup.shownMonth=new Date(shownMonth.getFullYear(),shownMonth.getMonth()+1,1)} }
            Row { width:parent.width;height:22;Repeater{model:["M","T","W","T","F","S","S"];Label{required property string modelData;width:parent.parent.width/7;text:modelData;color:win.muted;font.family:win.mono;font.pixelSize:11;horizontalAlignment:Text.AlignHCenter}} }
            Grid { width:parent.width;columns:7;rows:6
                Repeater { model:42
                    Rectangle { required property int index;width:parent.width/7;height:38;radius:6
                        property date cellDate:{var f=new Date(customRangePopup.shownMonth.getFullYear(),customRangePopup.shownMonth.getMonth(),1),off=(f.getDay()+6)%7;return new Date(f.getFullYear(),f.getMonth(),1-off+index)}
                        property string cellIso:iso(cellDate);property var bounds:statsBounds("custom",statsAnchor,customFrom,customTo)
                        color:cellIso===bounds.from||cellIso===bounds.to?theme.accent:(cellIso>bounds.from&&cellIso<bounds.to?"#3a3730":"transparent")
                        opacity:cellDate.getMonth()===customRangePopup.shownMonth.getMonth()?1:.35
                        Label { anchors.centerIn:parent;text:parent.cellDate.getDate();color:parent.cellIso===parent.bounds.from||parent.cellIso===parent.bounds.to?theme.accentForeground:win.text;font.family:win.mono;font.pixelSize:12 }
                        MouseArea { anchors.fill:parent;onClicked:{if(!customRangePopup.pickingEnd){customFrom=parent.cellDate;customTo=parent.cellDate;customRangePopup.pickingEnd=true}else{customTo=parent.cellDate;customRangePopup.pickingEnd=false}} }
                    }
                }
            }
            RowLayout { width:parent.width;Label{Layout.fillWidth:true;text:statsRangeLabel();color:win.muted;font.family:win.mono;font.pixelSize:11}AppButton{text:qsTr("Apply");primary:true;onClicked:{statsAnchor=customFrom;customRangeVisible=false;loadStatistics()}} }
        }
    }
    Rectangle { id:tip;objectName:"statsTooltip";z:3000;visible:false;property alias text:tipLabel.text;width:tipLabel.implicitWidth+24;height:38;radius:7;color:"#35322c";border.color:"#575249"
        Label { id:tipLabel;anchors.centerIn:parent;color:win.text;font.family:win.mono;font.pixelSize:11 }
    }

    // Quick add overlay
    Popup { id:quickPopup;visible:quickVisible;modal:true;focus:true;anchors.centerIn:Overlay.overlay;width:Math.min(760,win.width-80);height:245;closePolicy:Popup.NoAutoClose
        Overlay.modal: Rectangle { color:"#8c000000" }
        background:Rectangle{color:"#1c1b18";radius:Math.max(8,theme.radius);border.width:1;border.color:win.border;layer.enabled:true;layer.effect:MultiEffect{shadowEnabled:true;shadowOpacity:.4;shadowBlur:1}}
        onClosed:quickVisible=false
        ColumnLayout { anchors.fill:parent;anchors.margins:28;spacing:14
            RowLayout { Layout.fillWidth:true
                Label { text:quickBlock?qsTr("QUICK ADD · %1").arg(quickBlock.toUpperCase()):qsTr("QUICK ADD");color:win.muted;font.family:win.mono;font.pixelSize:12;font.letterSpacing:1.5;Layout.fillWidth:true }
                Label { text:"Ctrl K";color:win.muted;font.family:win.mono }
            }
            TextField { id:quickField;Layout.fillWidth:true;placeholderText:qsTr("e.g. read 20m after isha daily");font.pixelSize:20;onTextChanged:parseTimer.restart();onAccepted:if(text.trim()){var d=backend.parseDraft||{};d.title=d.title||text;maybeToast(d);backend.addQuick(text,quickBlock);quickVisible=false} }
            Label { Layout.fillWidth:true;text:backend.parseDraft.title?qsTr("%1 · %2 · %3 %4").arg(backend.parseDraft.title).arg(backend.parseDraft.block||qsTr("next prayer")).arg(backend.parseDraft.target||"").arg(backend.parseDraft.measure||""):qsTr("Describe what, when, and how often.");color:win.muted;font.family:win.mono;font.pixelSize:12;elide:Text.ElideRight }
            RowLayout { Layout.fillWidth:true;Item{Layout.fillWidth:true}
                AppButton { text:qsTr("Cancel");onClicked:quickVisible=false }
                AppButton { text:qsTr("Edit details");onClicked:{var d=backend.parseDraft||{};d.title=d.title||quickField.text;d.block=d.block||quickBlock||"fajr";openEntry(d);quickVisible=false} }
                AppButton { text:qsTr("Add  ↵");primary:true;enabled:quickField.text.trim()!=="";onClicked:{var d=backend.parseDraft||{};d.title=d.title||quickField.text;maybeToast(d);backend.addQuick(quickField.text,quickBlock);quickVisible=false} }
            }
        }
        Timer { id:parseTimer;interval:200;onTriggered:backend.parseQuick(quickField.text) }
    }

    // Detailed entry overlay
    Popup { id:entryPopup;objectName:"entryPopup";visible:entryVisible;modal:true;focus:true;anchors.centerIn:Overlay.overlay
        width:Math.min(1100,win.width-48);height:Math.min(820,win.height-48);padding:0;closePolicy:Popup.NoAutoClose
        property string block:editItem.block||"fajr";property string kind:editItem.measure==="count"?"counter":"task";property string repeat:editItem.repeat||"none";property var days:editItem.weekdays||[];property bool confirming:false
        readonly property bool weeklyValid:repeat!=="weekly"||days.length>0
        Overlay.modal: Rectangle { color:"#d9000000" }
        background:null
        onOpened:{block=editItem.block||"fajr";kind=editItem.measure==="count"?"counter":"task";repeat=editItem.repeat||"none";days=editItem.weekdays||[];confirming=false;titleField.text=editItem.title||"";goalField.text=editItem.target?String(editItem.target):""}
        function draft(){var goal=Number(goalField.text);return {id:editItem.id||"",title:titleField.text,block:block,measure:kind==="counter"?"count":goal>0?"minutes":"check",target:goal>0?goal:0,repeat:repeat,weekdays:repeat==="weekly"?days:[]}}
        function toggleDay(d){var a=days.slice();var i=a.indexOf(d);if(i>=0)a.splice(i,1);else a.push(d);days=a}
        function selectRepeat(id){if(id==="weekdays"){repeat="weekly";days=["mon","tue","wed","thu","fri"]}else{repeat=id;if(id==="weekly"&&days.length===0){var selected=Date.fromLocaleString(Qt.locale(),backend.selectedDay,"yyyy-MM-dd");days=[weekdayId(selected)]}}}
        Rectangle { anchors.fill:parent;color:"#151411";radius:Math.max(8,theme.radius);border.width:1;border.color:win.border }
        ColumnLayout { anchors.fill:parent;anchors.margins:1;spacing:0
            RowLayout { Layout.fillWidth:true;Layout.preferredHeight:72;Layout.leftMargin:22;Layout.rightMargin:22
                Label { text:editItem.id?qsTr("Edit entry"):qsTr("New entry");color:win.text;font.pixelSize:20;font.weight:Font.DemiBold;Layout.fillWidth:true }
                AppButton { visible:!!editItem.id;text:entryPopup.confirming?qsTr("Confirm delete"):qsTr("Delete");onClicked:{if(entryPopup.confirming){backend.removeItem(editItem.id);entryVisible=false}else entryPopup.confirming=true} }
                AppButton { text:qsTr("Cancel");onClicked:entryVisible=false }
                AppButton { text:qsTr("Save & add another");enabled:titleField.text.trim()!==""&&entryPopup.weeklyValid;onClicked:{var d=entryPopup.draft();maybeToast(d);backend.saveItem(d);editItem=({});titleField.text=""} }
                AppButton { text:qsTr("Save  ↵");primary:true;enabled:titleField.text.trim()!==""&&entryPopup.weeklyValid;onClicked:{var d=entryPopup.draft();maybeToast(d);backend.saveItem(d);entryVisible=false} }
            }
            Rectangle { Layout.fillWidth:true;height:1;color:win.border }
            RowLayout { Layout.fillWidth:true;Layout.fillHeight:true;Layout.margins:22;spacing:26
                ScrollView { id:entryScroll; Layout.fillWidth:true;Layout.fillHeight:true;Layout.minimumWidth:450;clip:true
                    Column { width:entryScroll.availableWidth;spacing:18
                        Rectangle { width:parent.width;height:108;radius:theme.radius;color:win.panel;border.color:win.border
                            Item { anchors.fill:parent;anchors.margins:24
                                Text { anchors.left:parent.left;anchors.verticalCenter:parent.verticalCenter;text:qsTr("Activity title…");color:win.muted;font.pixelSize:24;visible:titleField.text.length===0 }
                                TextInput { id:titleField;anchors.left:parent.left;anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;color:win.text;selectionColor:theme.accent;selectedTextColor:theme.accentForeground;font.pixelSize:28;clip:true;onAccepted:if(text.trim()&&entryPopup.weeklyValid){var d=entryPopup.draft();maybeToast(d);backend.saveItem(d);entryVisible=false} }
                                Rectangle { anchors.left:parent.left;anchors.right:parent.right;anchors.bottom:parent.bottom;height:1;color:win.border }
                            }
                        }
                        Rectangle { width:parent.width;height:150;radius:theme.radius;color:win.panel;border.color:win.border
                            ColumnLayout { anchors.fill:parent;anchors.margins:22;spacing:16
                                Label { text:qsTr("When");color:win.text;font.pixelSize:17;font.weight:Font.DemiBold }
                                RowLayout { spacing:8
                                    Repeater { model:win.blockIds
                                        ChoiceChip { required property string modelData;required property int index;text:win.blockNames[index];selected:entryPopup.block===modelData;onClicked:entryPopup.block=modelData }
                                    }
                                }
                            }
                        }
                        Rectangle { width:parent.width;height:160;radius:theme.radius;color:win.panel;border.color:win.border
                            ColumnLayout { anchors.fill:parent;anchors.margins:22;spacing:14
                                Label { text:qsTr("Kind");color:win.text;font.pixelSize:17;font.weight:Font.DemiBold }
                                RowLayout { spacing:8
                                    Repeater { model:[{id:"task",name:qsTr("Task")},{id:"counter",name:qsTr("Counter")}]
                                        ChoiceChip { required property var modelData;text:modelData.name;selected:entryPopup.kind===modelData.id;onClicked:entryPopup.kind=modelData.id }
                                    }
                                    Label { text:entryPopup.kind==="task"?qsTr("Time goal (min)"):qsTr("Goal");color:win.muted;Layout.leftMargin:10 }
                                    TextField { id:goalField;Layout.preferredWidth:110;placeholderText:qsTr("Optional");font.family:win.mono;inputMethodHints:Qt.ImhDigitsOnly;validator:IntValidator{bottom:1;top:99999} }
                                }
                            }
                        }
                        Rectangle { width:parent.width;height:entryPopup.repeat==="weekly"?220:150;radius:theme.radius;color:win.panel;border.color:win.border
                            ColumnLayout { anchors.fill:parent;anchors.margins:22;spacing:12
                                Label { text:qsTr("Repeat");color:win.text;font.pixelSize:17;font.weight:Font.DemiBold }
                                RowLayout { spacing:8
                                    Repeater { model:[{id:"none",name:qsTr("Once")},{id:"daily",name:qsTr("Daily")},{id:"weekdays",name:qsTr("Weekdays")},{id:"weekly",name:qsTr("Weekly")}]
                                        ChoiceChip { required property var modelData;text:modelData.name;selected:modelData.id!=="weekdays"&&entryPopup.repeat===modelData.id;onClicked:entryPopup.selectRepeat(modelData.id) }
                                    }
                                }
                                RowLayout { visible:entryPopup.repeat==="weekly";spacing:6
                                    Repeater { model:["mon","tue","wed","thu","fri","sat","sun"]
                                        ChoiceChip { required property string modelData;text:capitalized(modelData);selected:entryPopup.days.indexOf(modelData)>=0;onClicked:entryPopup.toggleDay(modelData) }
                                    }
                                }
                                Label { visible:entryPopup.repeat==="weekly"&&entryPopup.days.length===0;text:qsTr("Pick at least one day");color:win.muted;font.pixelSize:12 }
                            }
                        }
                    }
                }
                ColumnLayout { Layout.preferredWidth:340;Layout.minimumWidth:280;Layout.maximumWidth:360;Layout.fillHeight:true
                    Rectangle { Layout.fillWidth:true;height:245;radius:theme.radius;color:win.panel;border.color:win.border
                        ColumnLayout { anchors.fill:parent;anchors.margins:22;spacing:14
                            Label { text:qsTr("Preview");color:win.text;font.pixelSize:18;font.weight:Font.DemiBold }
                            Rectangle { Layout.fillWidth:true;height:1;color:win.border }
                            RowLayout { Layout.fillWidth:true;Label{text:qsTr("Next occurrence");color:win.muted;Layout.fillWidth:true}Label{text:backend.gregorianHeader(backend.selectedDay);color:win.text;font.family:win.mono} }
                            RowLayout { Layout.fillWidth:true;Label{text:qsTr("Occurrences this month");color:win.muted;Layout.fillWidth:true}Label{text:entryPopup.repeat==="daily"?30:entryPopup.repeat==="weekly"?Math.max(1,entryPopup.days.length*4):1;color:win.text;font.family:win.mono} }
                            RowLayout { Layout.fillWidth:true;Label{text:qsTr("Schedule");color:win.muted;Layout.fillWidth:true}Label{text:weekdayText(entryPopup.days,entryPopup.repeat);color:win.text;font.family:win.mono} }
                            RowLayout { Layout.fillWidth:true;Label{text:entryPopup.kind==="task"?qsTr("Time goal"):qsTr("Goal");color:win.muted;Layout.fillWidth:true}Label{text:goalField.text?(goalField.text+(entryPopup.kind==="task"?qsTr(" min"):"")):qsTr("None");color:win.text;font.family:win.mono} }
                        }
                    }
                    Item { Layout.fillHeight:true }
                }
            }
        }
    }

    Popup { visible:helpVisible;modal:true;focus:true;anchors.centerIn:Overlay.overlay;width:460;height:430;closePolicy:Popup.NoAutoClose
        background:Rectangle{color:win.panel;radius:theme.radius;border.color:win.border}
        ColumnLayout { anchors.fill:parent;anchors.margins:26;spacing:12
            Label { text:qsTr("Keyboard shortcuts");color:win.text;font.pixelSize:20;font.weight:Font.DemiBold }
            Repeater { model:[["Ctrl 1",qsTr("Today")],["Ctrl 2",qsTr("Routines")],["Ctrl 3",qsTr("Statistics")],["Ctrl 4",qsTr("Settings")],["Ctrl K",qsTr("Quick add")],["Enter",qsTr("Submit form")],["Esc",qsTr("Close top overlay / cancel drag")],["← / →",qsTr("Previous / next day")],["T",qsTr("Today")],["?",qsTr("This help")],["Ctrl Q / W",qsTr("Quit")],["Q",qsTr("Quit outside text fields")]]
                RowLayout { required property var modelData;Layout.fillWidth:true;Label{text:modelData[0];color:theme.accent;font.family:win.mono;Layout.preferredWidth:100}Label{text:modelData[1];color:win.text;Layout.fillWidth:true} }
            }
        }
    }
    Rectangle { visible:toastText!=="";z:2000;anchors.horizontalCenter:parent.horizontalCenter;anchors.bottom:parent.bottom;anchors.bottomMargin:28;width:toastLabel.implicitWidth+36;height:48;radius:theme.radius;color:"#302e28";border.color:win.border
        Label { id:toastLabel;anchors.centerIn:parent;text:toastText;color:win.text;textFormat:Text.PlainText }
    }
    Timer { id:toastTimer;interval:3500;onTriggered:toastText="" }
}
