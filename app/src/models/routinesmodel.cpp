#include "routinesmodel.h"

RoutinesModel::RoutinesModel(QObject *parent) : TypedListModel(parent) {
    setRoleNames({{RoutineRole, "routine"}, {DoneStripRole, "doneStrip"},
                  {DoneDaysRole, "doneDays"}, {MinutesRole, "minutes"}});
}

void RoutinesModel::setItems(const QVariantList &items) {
    m_items.clear();
    for (const QVariant &item : items) {
        const QString repeat = item.toMap().value("repeat").toString();
        if (!repeat.isEmpty() && repeat != "none") m_items.append(item);
    }
    rebuild();
}

void RoutinesModel::setHistory(const QDate &through, History history) {
    m_through = through;
    m_history = std::move(history);
    rebuild();
}

void RoutinesModel::rebuild() {
    QStringList ids;
    QVector<QVariantList> rows;
    for (const QVariant &item : std::as_const(m_items)) {
        const QString id = item.toMap().value("id").toString();
        const Activity activity = m_history.value(id);
        QVariantList strip;
        if (m_through.isValid())
            for (int offset = 1 - HistoryDays; offset <= 0; ++offset)
                strip.append(activity.done.contains(m_through.addDays(offset)));
        ids.append(id);
        rows.append({item, strip, int(activity.done.size()), activity.minutes});
    }
    setRows(ids, rows);
}
