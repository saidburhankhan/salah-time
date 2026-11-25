.pragma library

function multiline(event, field, submit, cancel) {
    if (event.key === Qt.Key_Escape) {
        event.accepted = true
        cancel()
    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
        event.accepted = true
        if (event.modifiers & Qt.ShiftModifier) {
            if (!field.readOnly) {
                field.remove(field.selectionStart, field.selectionEnd)
                field.insert(field.cursorPosition, "\n")
            }
        } else if (!(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))) {
            submit()
        }
    }
}


function route(event, win) {
    if (!win || event.modifiers & (Qt.ShiftModifier | Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
        return
    if (event.key === Qt.Key_Escape)
        event.accepted = win.closeTopOverlay()
    else if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && win.canSubmitForm()) {
        event.accepted = true
        win.submitForm()
    }
}
