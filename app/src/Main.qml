import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts

ApplicationWindow {
    id: win
    width: 1440; height: 900; minimumWidth: 1050; minimumHeight: 680
    visible: true; title: qsTr("Salah"); color: "#11110f"
    Material.theme: Material.Dark; Material.accent: theme.accent; Material.roundedScale: theme.radius
    property int page: 0
    property bool quickVisible: false
    property bool entryVisible: false
    property bool helpVisible: false
    property string quickBlock: ""
    property var editItem: ({})
    property int statsRange: 7
    property int countdown: backend.dayPlan.now && backend.dayPlan.now.next ? backend.dayPlan.now.next.remaining_seconds : 0
    property var prayerIds: ["fajr", "dhuhr", "asr", "maghrib", "isha"]
    property var prayerNames: [qsTr("Fajr"), qsTr("Dhuhr"), qsTr("Asr"), qsTr("Maghrib"), qsTr("Isha")]
    readonly property color panel: "#1b1a17"
    readonly property color card: "#201f1b"
    readonly property color border: "#34312a"
    readonly property color muted: "#8f8a80"
    readonly property color text: "#f1efe9"
    readonly property string mono: "monospace"

    function openQuick(block) { quickBlock=block||""; quickVisible=true; quickField.text=""; quickField.forceActiveFocus() }
    function openEntry(item) { editItem=item||({}); entryVisible=true }
    function blockItems(id) {
        var out=[]; var blocks=backend.dayPlan.blocks||[]
        for(var i=0;i<blocks.length;i++) if(blocks[i].start===id) return blocks[i].items||[]
        return out
    }
    function locationName() { var c=backend.config||{}; return c.location_name||c.name||(c.location?c.location.name:"")||qsTr("Location") }
    function methodName() { var c=backend.config||{}; return c.method_name||c.method||"—" }
    function detail(x) {
        var value=x.measure==="minutes"?qsTr("%1 / %2 min").arg(x.minutes||0).arg(x.target||0):x.measure==="count"?qsTr("%1 / %2").arg(x.count||0).arg(x.target||0):""
        var repeat=x.repeat==="daily"?qsTr("daily"):x.repeat==="weekly"?(x.weekdays||[]).join(" · "):qsTr("once")
        return value ? value+"  ·  "+repeat : repeat
    }
    function calendarDate(offset) {
        var base=Date.fromLocaleString(Qt.locale(),backend.selectedDay||backend.todayDay,"yyyy-MM-dd")
        var first=new Date(base.getFullYear(),base.getMonth(),1); var monday=(first.getDay()+6)%7
        return new Date(first.getFullYear(),first.getMonth(),1-monday+offset)
    }
    function iso(d) { return d.getFullYear()+"-"+String(d.getMonth()+1).padStart(2,"0")+"-"+String(d.getDate()).padStart(2,"0") }
    function isTyping() { var f=win.activeFocusItem; return f && f.inputMethodHints !== undefined }

    Shortcut { sequence: "Ctrl+1"; context: Qt.ApplicationShortcut; onActivated: page=0 }
    Shortcut { sequence: "Ctrl+2"; context: Qt.ApplicationShortcut; onActivated: page=1 }
    Shortcut { sequence: "Ctrl+3"; context: Qt.ApplicationShortcut; onActivated: {page=2;backend.loadStats(statsRange)} }
    Shortcut { sequence: "Ctrl+4"; context: Qt.ApplicationShortcut; onActivated: page=3 }
    Shortcut { sequence: "Ctrl+K"; context: Qt.ApplicationShortcut; onActivated: openQuick("") }
    Shortcut { sequence: "Left"; context: Qt.ApplicationShortcut; enabled: page===0&&!quickVisible&&!entryVisible; onActivated: backend.moveDay(-1) }
    Shortcut { sequence: "Right"; context: Qt.ApplicationShortcut; enabled: page===0&&!quickVisible&&!entryVisible; onActivated: backend.moveDay(1) }
    Shortcut { sequence: "T"; context: Qt.ApplicationShortcut; enabled: !isTyping(); onActivated: backend.jumpToday() }
    Shortcut { sequence: "?"; context: Qt.ApplicationShortcut; onActivated: helpVisible=!helpVisible }
    Shortcut { sequence: "Q"; context: Qt.ApplicationShortcut; enabled: !isTyping(); onActivated: Qt.quit() }
    Timer { interval:1000; running:true; repeat:true; onTriggered: if(countdown>0) countdown-- }
    Timer { interval:60000; running:true; repeat:true; onTriggered: backend.reload() }
    Connections { target:backend; function onChanged(){ if(backend.dayPlan.now&&backend.dayPlan.now.next) countdown=backend.dayPlan.now.next.remaining_seconds||0 } }
    onActiveChanged: if(active) backend.reload()

    RowLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.preferredWidth: 240; Layout.fillHeight: true; color: "#1c1b18"
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
                            Label { text:index===0?"◷":index===1?"↻":"▥"; color:win.page===index?win.text:win.muted; font.pixelSize:19; width:22 }
                            Label { text:modelData; color:win.page===index?win.text:"#c4c0b8"; font.pixelSize:16 }
                        }
                        MouseArea { anchors.fill:parent; onClicked:{win.page=index;if(index===2)backend.loadStats(statsRange)} }
                    }
                }
                Rectangle { Layout.fillWidth:true; height:1; color:win.border; Layout.topMargin:8; Layout.bottomMargin:8 }
                Rectangle { Layout.fillWidth:true; height:48; radius:theme.radius; color:win.page===3?win.card:"transparent"; border.color:win.page===3?win.border:"transparent"
                    Row { anchors.verticalCenter:parent.verticalCenter; leftPadding:14; spacing:13
                        Label { text:"⚙"; color:win.page===3?win.text:win.muted; font.pixelSize:19; width:22 }
                        Label { text:qsTr("Settings"); color:win.page===3?win.text:"#c4c0b8"; font.pixelSize:16 }
                    }
                    MouseArea { anchors.fill:parent; onClicked:win.page=3 }
                }
                Item { Layout.fillHeight:true }
                RowLayout { Layout.fillWidth:true; Layout.preferredHeight:48
                    Label { text:"⌖"; color:win.muted; font.pixelSize:20; Layout.leftMargin:12 }
                    Label { text:locationName(); color:"#c4c0b8"; font.pixelSize:15; Layout.fillWidth:true; elide:Text.ElideRight }
                }
            }
            Rectangle { anchors.right:parent.right; width:1; height:parent.height; color:win.border }
        }

        Item {
            Layout.fillWidth:true; Layout.fillHeight:true
            ColumnLayout { anchors.fill:parent; spacing:0
                Rectangle { Layout.fillWidth:true; height:74; color:"#171612"
                    RowLayout { anchors.fill:parent; anchors.leftMargin:32; anchors.rightMargin:32
                        ColumnLayout { visible:page===0; Layout.fillWidth:true; spacing:2
                            RowLayout { spacing:10
                                Label { text:(backend.selectedDay===backend.todayDay?qsTr("Today · "):"")+backend.hijriHeader(backend.selectedDay); color:win.text; font.pixelSize:22; font.weight:Font.DemiBold }
                                Label { text:backend.gregorianHeader(backend.selectedDay); color:win.muted; font.family:win.mono; font.pixelSize:15 }
                            }
                        }
                        Label { visible:page!==0; Layout.fillWidth:true; text:page===1?qsTr("Routines"):page===2?qsTr("Statistics"):qsTr("Settings"); color:win.text; font.pixelSize:22; font.weight:Font.DemiBold }
                        Button { visible:page===0; text:qsTr("+  Add"); highlighted:true; onClicked:openQuick(""); font.pixelSize:15 }
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
                    ScrollView { id:todayScroll; Layout.fillWidth:true; Layout.fillHeight:true; Layout.minimumWidth:600; clip:true
                        Column { width:todayScroll.availableWidth; spacing:10
                            Repeater { model:win.prayerIds
                                Column {
                                    id:blockColumn; required property string modelData; required property int index
                                    width:parent.width; spacing:8
                                    property var prayer:(backend.dayPlan.prayers||[])[index]||({})
                                    Rectangle { width:parent.width; height:60; radius:theme.radius; color:win.card; border.color:win.border
                                        RowLayout { anchors.fill:parent; anchors.leftMargin:18; anchors.rightMargin:16; spacing:14
                                            Rectangle { width:28;height:28;radius:14;color:blockColumn.prayer.marked?theme.accent:"transparent";border.width:2;border.color:blockColumn.prayer.marked?theme.accent:win.text
                                                Label { anchors.centerIn:parent; visible:!!blockColumn.prayer.marked; text:"✓"; color:theme.accentForeground; font.weight:Font.Bold }
                                                MouseArea { anchors.fill:parent; onClicked:backend.togglePrayer(blockColumn.modelData,blockColumn.prayer.marked||false) }
                                            }
                                            Label { text:blockColumn.prayer.name||win.prayerNames[blockColumn.index]; color:win.text; font.pixelSize:17; font.weight:Font.Medium; Layout.fillWidth:true }
                                            Label { text:blockColumn.prayer.time||"--:--"; color:"#d8d4cc"; font.family:win.mono; font.pixelSize:17 }
                                            Rectangle { visible:blockColumn.prayer.current||false; width:62;height:28;radius:14;color:"#4a2d26"
                                                Label { anchors.centerIn:parent;text:qsTr("● NOW");color:"#ef8068";font.family:win.mono;font.pixelSize:11 }
                                            }
                                        }
                                    }
                                    Repeater { model:blockItems(blockColumn.modelData)
                                        Rectangle { id:itemCard; required property var modelData; required property int index; width:parent.width; height:72; radius:theme.radius; color:"#1c1b18"; border.color:win.border
                                            Drag.active:drag.active; Drag.hotSpot.x:width/2; Drag.hotSpot.y:height/2; Drag.mimeData:{"text/item-id":modelData.id}
                                            DragHandler { id:drag }
                                            RowLayout { anchors.fill:parent; anchors.margins:12; spacing:12
                                                Label { text:"⠿"; color:win.muted; font.pixelSize:19 }
                                                Rectangle { width:24;height:24;radius:Math.min(5,theme.radius);color:modelData.state==="done"?theme.accent:"transparent";border.color:modelData.state==="done"?theme.accent:"#625e55"
                                                    Label { anchors.centerIn:parent;text:"✓";visible:modelData.state==="done";color:theme.accentForeground }
                                                    MouseArea { anchors.fill:parent;onClicked:backend.toggleDone(modelData.id,modelData.state) }
                                                }
                                                ColumnLayout { Layout.fillWidth:true;spacing:2
                                                    Label { text:modelData.title;color:modelData.state==="done"?win.muted:win.text;font.pixelSize:15;font.weight:Font.Medium;Layout.fillWidth:true;elide:Text.ElideRight;textFormat:Text.PlainText }
                                                    Label { text:detail(modelData);color:win.muted;font.family:win.mono;font.pixelSize:12 }
                                                }
                                                ToolButton { visible:modelData.measure==="minutes"&&backend.selectedDay===backend.todayDay;text:modelData.timer_started_at?"■":"▶";onClicked:backend.toggleTimer(modelData.id,!!modelData.timer_started_at) }
                                                ToolButton { visible:modelData.measure==="count";text:"+1";onClicked:backend.logOne(modelData.id) }
                                            }
                                            MouseArea { anchors.fill:parent; acceptedButtons:Qt.LeftButton; onDoubleClicked:openEntry(modelData); z:-1 }
                                            DropArea { anchors.fill:parent; onDropped:function(drop){backend.moveItem(drop.getDataAsString("text/item-id"),blockColumn.modelData,index)} }
                                        }
                                    }
                                    Rectangle { width:parent.width;height:36;color:"transparent"
                                        Label { anchors.left:parent.left;anchors.leftMargin:22;anchors.verticalCenter:parent.verticalCenter;text:qsTr("+  add");color:win.muted;font.pixelSize:14 }
                                        MouseArea { anchors.fill:parent;onClicked:openQuick(blockColumn.modelData) }
                                        DropArea { anchors.fill:parent; onDropped:function(drop){backend.moveItem(drop.getDataAsString("text/item-id"),blockColumn.modelData,blockItems(blockColumn.modelData).length)} }
                                    }
                                    Row { visible:blockColumn.index===0||blockColumn.index===4; height:26; spacing:12; leftPadding:18
                                        Rectangle { width:8;height:8;radius:4;color:"#555148";anchors.verticalCenter:parent.verticalCenter }
                                        Label { text:blockColumn.index===0?qsTr("Sunrise  %1").arg(backend.dayPlan.sunrise||"--:--"):qsTr("Sleep  %1").arg((backend.dayPlan.sleep||{}).time||"--:--");color:win.muted;font.family:win.mono;font.pixelSize:12;anchors.verticalCenter:parent.verticalCenter }
                                    }
                                }
                            }
                        }
                    }
                    ColumnLayout { Layout.preferredWidth:320; Layout.minimumWidth:320; Layout.maximumWidth:320; Layout.fillHeight:true; spacing:18
                        Rectangle { Layout.fillWidth:true; height:260; radius:theme.radius; color:win.panel; border.color:win.border
                            ColumnLayout { anchors.fill:parent;anchors.margins:22;spacing:8
                                RowLayout { Layout.fillWidth:true
                                    Label { text:qsTr("Now");color:win.text;font.pixelSize:18;font.weight:Font.DemiBold }
                                    Label { Layout.fillWidth:true;text:(backend.dayPlan.now||{}).block?String(backend.dayPlan.now.block).toUpperCase():"";color:win.muted;font.family:win.mono;horizontalAlignment:Text.AlignRight }
                                }
                                Rectangle { Layout.fillWidth:true;height:1;color:win.border;Layout.topMargin:6;Layout.bottomMargin:18 }
                                Label { Layout.alignment:Qt.AlignHCenter;text:(backend.dayPlan.now||{}).sleeping?qsTr("RESTING"):qsTr("NEXT · %1").arg(((backend.dayPlan.now||{}).next||{}).name||"—");color:win.muted;font.family:win.mono;font.pixelSize:13;font.letterSpacing:1 }
                                Label { Layout.alignment:Qt.AlignHCenter;text:(backend.dayPlan.now||{}).sleeping?qsTr("Sleeping"):Math.floor(countdown/3600)+"h "+String(Math.floor((countdown%3600)/60)).padStart(2,"0")+"m";color:win.text;font.pixelSize:34;font.weight:Font.Medium }
                                Label { Layout.alignment:Qt.AlignHCenter;text:qsTr("at %1").arg(((backend.dayPlan.now||{}).next||{}).time||"--:--");color:win.muted;font.family:win.mono;font.pixelSize:14 }
                                Item { Layout.fillHeight:true }
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
                                        Label { text:String(modelData.block).toUpperCase()+"  ·  "+(modelData.repeat==="daily"?qsTr("Daily"):(modelData.weekdays||[]).join(" · "));color:win.muted;font.family:win.mono;font.pixelSize:12 }
                                        Label { text:qsTr("%1 of %2 days").arg(modelData.done_days).arg(modelData.planned_days);color:"#c7c2b9";font.pixelSize:13 }
                                    }
                                    RowLayout { Layout.fillWidth:true;Layout.fillHeight:true;spacing:5
                                        Repeater { model:modelData.series||[]
                                            Item { required property var modelData;Layout.fillWidth:true;Layout.fillHeight:true
                                                Rectangle { anchors.horizontalCenter:parent.horizontalCenter;anchors.bottom:parent.bottom;width:modelData.measure==="check"?7:Math.max(5,parent.width-3);height:modelData.done?Math.max(7,Math.min(parent.height-12,(modelData.minutes||modelData.count||1)*2)):5;radius:3;color:modelData.done?theme.accent:"#38352f" }
                                            }
                                        }
                                    }
                                    ColumnLayout { Layout.preferredWidth:90
                                        Label { text:modelData.measure==="minutes"?qsTr("%1 min").arg(modelData.minutes):modelData.measure==="count"?String(modelData.count):String(modelData.done_days);color:win.text;font.family:win.mono;font.pixelSize:18;Layout.alignment:Qt.AlignRight }
                                        Label { text:qsTr("total");color:win.muted;Layout.alignment:Qt.AlignRight }
                                    }
                                }
                                MouseArea { anchors.fill:parent;onClicked:openEntry(modelData) }
                            }
                        }
                    }
                }

                // Statistics
                ScrollView { id:statsScroll; visible:page===2;Layout.fillWidth:true;Layout.fillHeight:true;clip:true;Layout.margins:30
                    Column { width:statsScroll.availableWidth;spacing:20
                        Row { spacing:8
                            Repeater { model:[7,30]
                                Button { required property int modelData;text:qsTr("%1 days").arg(modelData);highlighted:statsRange===modelData;onClicked:{statsRange=modelData;backend.loadStats(modelData)} }
                            }
                        }
                        Rectangle { width:parent.width;height:280;radius:theme.radius;color:win.panel;border.color:win.border
                            ColumnLayout { anchors.fill:parent;anchors.margins:22;spacing:10
                                RowLayout { Layout.fillWidth:true
                                    Label { text:qsTr("Prayer completion");color:win.text;font.pixelSize:18;font.weight:Font.DemiBold;Layout.fillWidth:true }
                                    Label { text:qsTr("%1 / %2 overall").arg(backend.stats.marked_total||0).arg(backend.stats.prayer_slots||0);color:win.muted;font.family:win.mono }
                                }
                                Repeater { model:backend.stats.prayers||[]
                                    RowLayout { required property var modelData;Layout.fillWidth:true;spacing:14
                                        Label { text:String(modelData.prayer).charAt(0).toUpperCase()+String(modelData.prayer).slice(1);color:"#d8d4cc";Layout.preferredWidth:90 }
                                        Rectangle { Layout.fillWidth:true;height:10;radius:5;color:"#38352f"
                                            Rectangle { width:parent.width*(modelData.days?modelData.marked_days/modelData.days:0);height:parent.height;radius:5;color:theme.accent }
                                        }
                                        Label { text:modelData.marked_days+" / "+modelData.days;color:win.muted;font.family:win.mono;Layout.preferredWidth:55;horizontalAlignment:Text.AlignRight }
                                    }
                                }
                            }
                        }
                        Label { text:qsTr("Activities");color:win.text;font.pixelSize:18;font.weight:Font.DemiBold }
                        Repeater { model:(backend.stats.items||[]).slice().sort(function(a,b){return String(a.title).localeCompare(String(b.title))})
                            Rectangle { required property var modelData;width:parent.width;height:76;radius:theme.radius;color:win.panel;border.color:win.border
                                RowLayout { anchors.fill:parent;anchors.margins:18
                                    ColumnLayout { Layout.fillWidth:true
                                        Label { text:modelData.title;color:win.text;font.pixelSize:15;font.weight:Font.Medium }
                                        Label { text:String(modelData.block).toUpperCase()+"  ·  "+modelData.done_days+" / "+modelData.planned_days+" "+qsTr("days");color:win.muted;font.family:win.mono;font.pixelSize:12 }
                                    }
                                    Label { text:modelData.measure==="minutes"?qsTr("%1 min").arg(modelData.minutes):modelData.measure==="count"?String(modelData.count):String(modelData.done_days);color:win.text;font.family:win.mono;font.pixelSize:18 }
                                }
                            }
                        }
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
                                    Button { text:qsTr("Save");highlighted:true;onClicked:backend.setSleepTime(sleepField.text) }
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

    // Quick add overlay
    Popup { id:quickPopup;visible:quickVisible;modal:true;focus:true;anchors.centerIn:Overlay.overlay;width:Math.min(760,win.width-80);height:245;closePolicy:Popup.NoAutoClose
        background:Rectangle{color:"#1c1b18";radius:theme.radius;border.color:win.border}
        onClosed:quickVisible=false
        ColumnLayout { anchors.fill:parent;anchors.margins:28;spacing:14
            RowLayout { Layout.fillWidth:true
                Label { text:quickBlock?qsTr("QUICK ADD · %1").arg(quickBlock.toUpperCase()):qsTr("QUICK ADD");color:win.muted;font.family:win.mono;font.pixelSize:12;font.letterSpacing:1.5;Layout.fillWidth:true }
                Label { text:"Ctrl K";color:win.muted;font.family:win.mono }
            }
            TextField { id:quickField;Layout.fillWidth:true;placeholderText:qsTr("e.g. read 20m after isha daily");font.pixelSize:20;onTextChanged:parseTimer.restart();onAccepted:if(text.trim()){backend.addQuick(text,quickBlock);quickVisible=false} }
            Label { Layout.fillWidth:true;text:backend.parseDraft.title?qsTr("%1 · %2 · %3 %4").arg(backend.parseDraft.title).arg(backend.parseDraft.block||qsTr("next prayer")).arg(backend.parseDraft.target||"").arg(backend.parseDraft.measure||""):qsTr("Describe what, when, and how often.");color:win.muted;font.family:win.mono;font.pixelSize:12;elide:Text.ElideRight }
            RowLayout { Layout.fillWidth:true;Item{Layout.fillWidth:true}
                Button { text:qsTr("Edit details");onClicked:{var d=backend.parseDraft||{};d.title=d.title||quickField.text;d.block=d.block||quickBlock||"fajr";openEntry(d);quickVisible=false} }
                Button { text:qsTr("Add  ↵");highlighted:true;enabled:quickField.text.trim()!=="";onClicked:{backend.addQuick(quickField.text,quickBlock);quickVisible=false} }
            }
        }
        Timer { id:parseTimer;interval:200;onTriggered:backend.parseQuick(quickField.text) }
    }

    // Detailed entry overlay
    Popup { id:entryPopup;visible:entryVisible;modal:true;focus:true;anchors.centerIn:Overlay.overlay;width:win.width-44;height:win.height-44;closePolicy:Popup.NoAutoClose
        property string block:editItem.block||"fajr";property string measure:editItem.measure||"check";property string repeat:editItem.repeat||"none";property var days:editItem.weekdays||[];property bool confirming:false
        background:Rectangle{color:"#151411";radius:theme.radius;border.color:win.border}
        onOpened:{block=editItem.block||"fajr";measure=editItem.measure||"check";repeat=editItem.repeat||"none";days=editItem.weekdays||[];confirming=false;titleField.text=editItem.title||"";amount.value=editItem.target||1}
        function draft(){return {id:editItem.id||"",title:titleField.text,block:block,measure:measure,target:amount.value,repeat:repeat,weekdays:days}}
        function toggleDay(d){var a=days.slice();var i=a.indexOf(d);if(i>=0)a.splice(i,1);else a.push(d);days=a}
        ColumnLayout { anchors.fill:parent;spacing:0
            RowLayout { Layout.fillWidth:true;Layout.preferredHeight:72;Layout.leftMargin:22;Layout.rightMargin:22
                Label { text:editItem.id?qsTr("Edit entry"):qsTr("New entry");color:win.text;font.pixelSize:20;font.weight:Font.DemiBold;Layout.fillWidth:true }
                Button { visible:!!editItem.id;text:entryPopup.confirming?qsTr("Confirm delete"):qsTr("Delete");onClicked:{if(entryPopup.confirming){backend.removeItem(editItem.id);entryVisible=false}else entryPopup.confirming=true} }
                Button { text:qsTr("Cancel");onClicked:entryVisible=false }
                Button { text:qsTr("Save & add another");onClicked:{backend.saveItem(entryPopup.draft());editItem=({});titleField.text=""} }
                Button { text:qsTr("Save  ↵");highlighted:true;onClicked:{backend.saveItem(entryPopup.draft());entryVisible=false} }
            }
            Rectangle { Layout.fillWidth:true;height:1;color:win.border }
            RowLayout { Layout.fillWidth:true;Layout.fillHeight:true;Layout.margins:22;spacing:26
                ScrollView { id:entryScroll; Layout.fillWidth:true;Layout.fillHeight:true;Layout.minimumWidth:650;clip:true
                    Column { width:entryScroll.availableWidth;spacing:18
                        Rectangle { width:parent.width;height:108;radius:theme.radius;color:win.panel;border.color:win.border
                            TextField { id:titleField;anchors.fill:parent;anchors.margins:22;placeholderText:qsTr("Activity title…");font.pixelSize:25;verticalAlignment:TextInput.AlignVCenter }
                        }
                        Rectangle { width:parent.width;height:150;radius:theme.radius;color:win.panel;border.color:win.border
                            ColumnLayout { anchors.fill:parent;anchors.margins:22;spacing:16
                                Label { text:qsTr("When");color:win.text;font.pixelSize:17;font.weight:Font.DemiBold }
                                RowLayout { spacing:8
                                    Repeater { model:win.prayerIds
                                        Button { required property string modelData;required property int index;text:win.prayerNames[index];highlighted:entryPopup.block===modelData;onClicked:entryPopup.block=modelData }
                                    }
                                }
                            }
                        }
                        Rectangle { width:parent.width;height:160;radius:theme.radius;color:win.panel;border.color:win.border
                            ColumnLayout { anchors.fill:parent;anchors.margins:22;spacing:14
                                Label { text:qsTr("Measure");color:win.text;font.pixelSize:17;font.weight:Font.DemiBold }
                                RowLayout { spacing:8
                                    Repeater { model:[{id:"check",name:qsTr("Tick")},{id:"minutes",name:qsTr("Minutes")},{id:"count",name:qsTr("Count")}]
                                        Button { required property var modelData;text:modelData.name;highlighted:entryPopup.measure===modelData.id;onClicked:entryPopup.measure=modelData.id }
                                    }
                                    SpinBox { id:amount;visible:entryPopup.measure!=="check";from:1;to:999;value:1;editable:true;Layout.leftMargin:10 }
                                }
                            }
                        }
                        Rectangle { width:parent.width;height:entryPopup.repeat==="weekly"?220:150;radius:theme.radius;color:win.panel;border.color:win.border
                            ColumnLayout { anchors.fill:parent;anchors.margins:22;spacing:12
                                Label { text:qsTr("Repeat");color:win.text;font.pixelSize:17;font.weight:Font.DemiBold }
                                RowLayout { spacing:8
                                    Repeater { model:[{id:"none",name:qsTr("Once")},{id:"daily",name:qsTr("Daily")},{id:"weekdays",name:qsTr("Weekdays")},{id:"weekly",name:qsTr("Weekly")}]
                                        Button { required property var modelData;text:modelData.name;highlighted:(modelData.id==="weekdays"?entryPopup.repeat==="weekly"&&entryPopup.days.join(",")==="mon,tue,wed,thu,fri":entryPopup.repeat===modelData.id);onClicked:{if(modelData.id==="weekdays"){entryPopup.repeat="weekly";entryPopup.days=["mon","tue","wed","thu","fri"]}else entryPopup.repeat=modelData.id} }
                                    }
                                }
                                RowLayout { visible:entryPopup.repeat==="weekly";spacing:6
                                    Repeater { model:["mon","tue","wed","thu","fri","sat","sun"]
                                        Button { required property string modelData;text:modelData.charAt(0).toUpperCase()+modelData.slice(1);highlighted:entryPopup.days.indexOf(modelData)>=0;onClicked:entryPopup.toggleDay(modelData) }
                                    }
                                }
                            }
                        }
                    }
                }
                ColumnLayout { Layout.preferredWidth:360;Layout.minimumWidth:360;Layout.maximumWidth:360;Layout.fillHeight:true
                    Rectangle { Layout.fillWidth:true;height:245;radius:theme.radius;color:win.panel;border.color:win.border
                        ColumnLayout { anchors.fill:parent;anchors.margins:22;spacing:14
                            Label { text:qsTr("Preview");color:win.text;font.pixelSize:18;font.weight:Font.DemiBold }
                            Rectangle { Layout.fillWidth:true;height:1;color:win.border }
                            RowLayout { Layout.fillWidth:true;Label{text:qsTr("Next occurrence");color:win.muted;Layout.fillWidth:true}Label{text:backend.gregorianHeader(backend.selectedDay);color:win.text;font.family:win.mono} }
                            RowLayout { Layout.fillWidth:true;Label{text:qsTr("Occurrences this month");color:win.muted;Layout.fillWidth:true}Label{text:entryPopup.repeat==="daily"?30:entryPopup.repeat==="weekly"?Math.max(1,entryPopup.days.length*4):1;color:win.text;font.family:win.mono} }
                            RowLayout { Layout.fillWidth:true;Label{text:qsTr("Amount");color:win.muted;Layout.fillWidth:true}Label{text:entryPopup.measure==="check"?qsTr("Tick"):amount.value+" "+entryPopup.measure;color:win.text;font.family:win.mono} }
                        }
                    }
                    Item { Layout.fillHeight:true }
                }
            }
        }
    }

    Popup { visible:helpVisible;modal:true;focus:true;anchors.centerIn:Overlay.overlay;width:440;height:390;closePolicy:Popup.CloseOnEscape|Popup.CloseOnPressOutside
        background:Rectangle{color:win.panel;radius:theme.radius;border.color:win.border}
        ColumnLayout { anchors.fill:parent;anchors.margins:26;spacing:12
            Label { text:qsTr("Keyboard shortcuts");color:win.text;font.pixelSize:20;font.weight:Font.DemiBold }
            Repeater { model:[["Ctrl 1",qsTr("Today")],["Ctrl 2",qsTr("Routines")],["Ctrl 3",qsTr("Statistics")],["Ctrl 4",qsTr("Settings")],["Ctrl K",qsTr("Quick add")],["← / →",qsTr("Previous / next day")],["T",qsTr("Today")],["?",qsTr("This help")],["Q",qsTr("Quit")]]
                RowLayout { required property var modelData;Layout.fillWidth:true;Label{text:modelData[0];color:theme.accent;font.family:win.mono;Layout.preferredWidth:100}Label{text:modelData[1];color:win.text;Layout.fillWidth:true} }
            }
        }
    }
}
