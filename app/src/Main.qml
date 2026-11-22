import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts

ApplicationWindow {
    id: win
    width: 980; height: 720; minimumWidth: 720; minimumHeight: 520
    visible: true; title: qsTr("Salah"); color: "#101114"
    Material.theme: Material.Dark; Material.accent: theme.accent; Material.roundedScale: theme.radius
    property bool routinesView: false
    property bool helpVisible: false
    property int selectedRow: 0
    property var blocks: ["fajr", "dhuhr", "asr", "maghrib", "isha"]
    property var blockNames: [qsTr("Fajr"), qsTr("Dhuhr"), qsTr("Asr"), qsTr("Maghrib"), qsTr("Isha")]
    function selectedDate() { translator.language; return Date.fromLocaleString(Qt.locale(), backend.selectedDay, "yyyy-MM-dd") }
    function summary() { var done=0, mins=0; for(var i=0;i<backend.selectedItems.length;i++){var x=backend.selectedItems[i];if(x.state==="done")done++;mins+=x.minutes} return qsTr("%1 of %2 done · %3 min").arg(done).arg(backend.selectedItems.length).arg(mins) }
    function moveSelection(d) { selectedRow=Math.max(0,Math.min(backend.selectedItems.length-1,selectedRow+d)) }
    Shortcut { sequence: "?"; context: Qt.ApplicationShortcut; onActivated: helpVisible=!helpVisible }
    Shortcut { sequence: "Q"; context: Qt.ApplicationShortcut; onActivated: Qt.quit() }
    Shortcut { sequence: "Tab"; context: Qt.ApplicationShortcut; onActivated: routinesView=!routinesView }
    Shortcut { sequences: ["Left", "H"]; context: Qt.ApplicationShortcut; enabled: !routinesView; onActivated: backend.moveDay(-1) }
    Shortcut { sequences: ["Right", "L"]; context: Qt.ApplicationShortcut; enabled: !routinesView; onActivated: backend.moveDay(1) }
    Shortcut { sequence: "T"; context: Qt.ApplicationShortcut; onActivated: backend.jumpToday() }
    Shortcut { sequence: "R"; context: Qt.ApplicationShortcut; onActivated: backend.reload() }
    Shortcut { sequence: "N"; context: Qt.ApplicationShortcut; onActivated: addDialog.open() }
    Shortcut { sequences: ["Down", "J"]; context: Qt.ApplicationShortcut; enabled: !routinesView; onActivated: moveSelection(1) }
    Shortcut { sequences: ["Up", "K"]; context: Qt.ApplicationShortcut; enabled: !routinesView; onActivated: moveSelection(-1) }
    Shortcut { sequence: "Space"; context: Qt.ApplicationShortcut; enabled: !routinesView && backend.selectedItems.length>0; onActivated: { var x=backend.selectedItems[selectedRow]; backend.toggleDone(x.id,x.state) } }
    Timer { interval: 60000; running: true; repeat: true; onTriggered: backend.reload() }
    onActiveChanged: if (active) backend.reload()

    Item {
        anchors.fill: parent; anchors.margins: 24
        LayoutMirroring.enabled: Qt.application.layoutDirection===Qt.RightToLeft; LayoutMirroring.childrenInherit: true
        ColumnLayout {
            anchors.fill: parent; spacing: 16
            RowLayout {
                Layout.fillWidth: true
                ColumnLayout { Layout.fillWidth: true; spacing: 2
                    Label { text: routinesView ? qsTr("Routines") : Qt.locale().toString(selectedDate(), "dddd, d MMMM yyyy"); font.pixelSize: 25; font.weight: Font.DemiBold }
                    Label { text: routinesView ? qsTr("The last 14 days") : summary(); color: "#9b9da5"; font.pixelSize: 14 }
                }
                Label { text: routinesView ? qsTr("TAB  DAY") : qsTr("TAB  ROUTINES"); color: theme.accent; font.pixelSize: 12 }
            }
            RowLayout {
                visible: !routinesView; Layout.fillWidth: true; spacing: 8
                Repeater { model: backend.days
                    Rectangle {
                        required property var modelData; required property int index
                        visible: index >= Math.max(0, backend.days.length-7)
                        Layout.fillWidth: visible; Layout.preferredHeight: 58; radius: theme.radius
                        color: modelData.day===backend.selectedDay ? theme.accent : "#1b1c21"
                        Column { anchors.centerIn: parent; spacing: 3
                            Label { anchors.horizontalCenter: parent.horizontalCenter; text: Qt.locale().toString(Date.fromLocaleString(Qt.locale(),modelData.day,"yyyy-MM-dd"),"ddd d"); color: modelData.day===backend.selectedDay?theme.accentForeground:"#ececf0"; font.weight: Font.Medium }
                            Label { anchors.horizontalCenter: parent.horizontalCenter; text: qsTr("%1 / %2").arg(modelData.done).arg(modelData.planned); color: modelData.day===backend.selectedDay?theme.accentForeground:"#92949c"; font.pixelSize: 12 }
                        }
                        MouseArea { anchors.fill: parent; onClicked: backend.selectDay(modelData.day) }
                    }
                }
            }
            Rectangle { visible: backend.error!==""; Layout.fillWidth: true; height: errorText.implicitHeight+20; color: "#3b2024"; radius: theme.radius
                Label { id:errorText; anchors.fill: parent; anchors.margins: 10; text: backend.error; color: "#ffb4b8"; wrapMode: Text.Wrap; textFormat: Text.PlainText }
            }
            ScrollView { visible: !routinesView; Layout.fillWidth: true; Layout.fillHeight: true; clip: true
                Column { width: parent.width; spacing: 12
                    Repeater { model: win.blocks
                        Rectangle {
                            required property string modelData; required property int index
                            property string blockKey: modelData
                            width: parent.width; height: section.implicitHeight+24; color: "#18191e"; radius: theme.radius
                            Column { id:section; anchors.left: parent.left; anchors.right: parent.right; anchors.margins: 12; anchors.verticalCenter: parent.verticalCenter; spacing: 6
                                Label { text: win.blockNames[index]; color: theme.accent; font.pixelSize: 13; font.weight: Font.DemiBold }
                                Repeater { model: backend.selectedItems
                                    Rectangle {
                                        required property var modelData; required property int index
                                        visible: modelData.block===section.parent.blockKey
                                        width: parent.width; height: visible?42:0; radius: Math.min(4,theme.radius)
                                        color: index===selectedRow ? "#292b32" : "transparent"
                                        RowLayout { anchors.fill: parent; anchors.leftMargin: 8; anchors.rightMargin: 8
                                            Label { text: modelData.state==="done"?"✓":modelData.state==="missed"?"—":"○"; color: modelData.state==="done"?theme.accent:"#858790"; font.pixelSize: 18 }
                                            Label { Layout.fillWidth:true; text:modelData.title; color:modelData.state==="missed"?"#777982":"#eeeeF2"; textFormat:Text.PlainText; elide:Text.ElideRight }
                                            Label { visible:modelData.measure!=="check"; text:modelData.measure==="minutes"?qsTr("%1 / %2 min").arg(modelData.minutes).arg(modelData.target||"—"):qsTr("%1 / %2").arg(modelData.count).arg(modelData.target||"—"); color:"#9b9da5" }
                                        }
                                        MouseArea { anchors.fill: parent; onClicked: selectedRow=index }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            ScrollView { visible: routinesView; Layout.fillWidth:true; Layout.fillHeight:true; clip:true
                Column { width:parent.width; spacing:10
                    Repeater { model:backend.routines
                        Rectangle { required property var modelData; width:parent.width; height:100; color:"#18191e"; radius:theme.radius
                            RowLayout { anchors.fill:parent; anchors.margins:14; spacing:16
                                ColumnLayout { Layout.preferredWidth:180
                                    Label { text:modelData.title; font.pixelSize:15; elide:Text.ElideRight; Layout.fillWidth:true }
                                    Label { text:modelData.measure==="minutes"?qsTr("%1 min total").arg(modelData.total):qsTr("%1 total").arg(modelData.total); color:"#999ba3" }
                                }
                                RowLayout { id: chart; property string routineMeasure: modelData.measure; Layout.fillWidth:true; Layout.fillHeight:true; spacing:5
                                    Repeater { model:modelData.values
                                        Item { required property int modelData; Layout.fillWidth:true; Layout.fillHeight:true
                                            Rectangle { anchors.horizontalCenter:parent.horizontalCenter; anchors.bottom:parent.bottom; width:chart.routineMeasure==="check"?8:Math.max(5,parent.width-2); height:chart.routineMeasure==="check"?8:Math.max(5,Math.min(parent.height,modelData*(modelData>0?3:0))); radius:chart.routineMeasure==="check"?4:Math.min(3,theme.radius); color:modelData>0?theme.accent:"#303139" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Dialog { id:addDialog; title:qsTr("Add item"); modal:true; anchors.centerIn:parent; width:420; standardButtons:Dialog.Ok|Dialog.Cancel
        onOpened: { blockBox.currentIndex=Math.max(0,blocks.indexOf(backend.planningBlock)); titleField.forceActiveFocus() }
        onAccepted: backend.addItem(titleField.text,blocks[blockBox.currentIndex],daily.checked,["check","minutes","count"][measure.currentIndex],parseInt(target.text)||0)
        ColumnLayout { width:parent.width
            TextField { id:titleField; Layout.fillWidth:true; placeholderText:qsTr("Title"); onAccepted:addDialog.accept() }
            ComboBox { id:blockBox; Layout.fillWidth:true; model:blockNames }
            Switch { id:daily; text:qsTr("Daily") }
            ComboBox { id:measure; Layout.fillWidth:true; model:[qsTr("Tick"),qsTr("Minutes"),qsTr("Count")] }
            TextField { id:target; visible:measure.currentIndex>0; Layout.fillWidth:true; placeholderText:qsTr("Target (optional)"); inputMethodHints:Qt.ImhDigitsOnly }
        }
    }
    Popup { visible:helpVisible; modal:true; anchors.centerIn:Overlay.overlay; width:460; height:360; closePolicy:Popup.CloseOnEscape|Popup.CloseOnPressOutside; background:Rectangle{color:"#202126";radius:theme.radius}
        Column { anchors.fill:parent; anchors.margins:24; spacing:12
            Label { text:qsTr("Keyboard shortcuts"); font.pixelSize:20; font.weight:Font.DemiBold }
            Label { text:qsTr("←/→ or H/L   Previous / next day\nT             Today\nTab           Day / routines\n↑/↓ or J/K    Select item\nSpace         Done / undo (today)\nN             New item\nR             Reload\n?             Shortcuts\nQ             Quit"); lineHeight:1.35 }
        }
    }
}
