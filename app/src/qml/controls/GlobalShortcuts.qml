import QtQuick
import QtQuick.Controls

Item {
    id: shortcuts
    property var win

    Shortcut { sequence: "Ctrl+1"; context: Qt.ApplicationShortcut; onActivated: shortcuts.win.page = 0 }
    Shortcut { sequence: "Ctrl+2"; context: Qt.ApplicationShortcut; onActivated: shortcuts.win.page = 1 }
    Shortcut { sequence: "Ctrl+3"; context: Qt.ApplicationShortcut; onActivated: { shortcuts.win.page = 2; shortcuts.win.loadStatistics() } }
    Shortcut { sequence: "Ctrl+4"; context: Qt.ApplicationShortcut; onActivated: shortcuts.win.page = 3 }
    Shortcut { sequence: "Ctrl+K"; context: Qt.ApplicationShortcut; onActivated: shortcuts.win.openQuick("") }
    Shortcut { sequence: "Left"; context: Qt.ApplicationShortcut; enabled: shortcuts.win.page === 0 && !shortcuts.win.quickVisible && !shortcuts.win.entryVisible && !shortcuts.win.isTyping(); onActivated: backend.moveDay(-1) }
    Shortcut { sequence: "Right"; context: Qt.ApplicationShortcut; enabled: shortcuts.win.page === 0 && !shortcuts.win.quickVisible && !shortcuts.win.entryVisible && !shortcuts.win.isTyping(); onActivated: backend.moveDay(1) }
    Shortcut { sequence: "Left"; context: Qt.ApplicationShortcut; enabled: shortcuts.win.page === 2 && !shortcuts.win.customRangeVisible && !shortcuts.win.isTyping(); onActivated: shortcuts.win.stepStats(-1) }
    Shortcut { sequence: "Right"; context: Qt.ApplicationShortcut; enabled: shortcuts.win.page === 2 && !shortcuts.win.customRangeVisible && !shortcuts.win.isTyping(); onActivated: shortcuts.win.stepStats(1) }
    Shortcut { sequence: "T"; context: Qt.ApplicationShortcut; enabled: !shortcuts.win.isTyping(); onActivated: backend.jumpToday() }
    Shortcut { sequence: "?"; context: Qt.ApplicationShortcut; onActivated: shortcuts.win.helpVisible = !shortcuts.win.helpVisible }
    Shortcut { sequence: "Q"; context: Qt.ApplicationShortcut; enabled: !shortcuts.win.isTyping(); onActivated: Qt.quit() }
    Shortcut { sequence: "Ctrl+Q"; context: Qt.ApplicationShortcut; onActivated: Qt.quit() }
    Shortcut { sequence: "Ctrl+W"; context: Qt.ApplicationShortcut; onActivated: Qt.quit() }
    Timer {
        interval: 1000
        running: true
        repeat: true
        onTriggered: {
            shortcuts.win.countdown = shortcuts.win.countdownFromNext()
            shortcuts.win.timerNow = Date.now()
        }
    }
    Timer {
        interval: 60000
        running: true
        repeat: true
        onTriggered: backend.reload()
    }
    Connections {
        target: backend.nowState
        function onChanged() {
            shortcuts.win.countdown = shortcuts.win.countdownFromNext()
        }
    }
    Shortcut {
        sequences: ["Return", "Enter"]
        context: Qt.WindowShortcut
        enabled: shortcuts.win.canSubmitForm()
        onActivated: shortcuts.win.submitForm()
    }
    Shortcut {
        sequence: "Escape"
        context: Qt.ApplicationShortcut
        onActivated: {
            if (shortcuts.win.customRangeVisible)
                shortcuts.win.customRangeVisible = false
            else
                shortcuts.win.closeTopOverlay()
        }
    }
}
