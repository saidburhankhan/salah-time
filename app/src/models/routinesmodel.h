#pragma once

#include "typedlistmodel.h"

#include <QDate>
#include <QSet>

class RoutinesModel : public TypedListModel {
    Q_OBJECT
public:
    enum Role { RoutineRole = Qt::UserRole + 1, DoneStripRole, DoneDaysRole, MinutesRole };
    static constexpr int HistoryDays = 14;
    struct Activity {
        QSet<QDate> done;
        int minutes = 0;
    };
    using History = QHash<QString, Activity>;

    explicit RoutinesModel(QObject *parent = nullptr);
    void setItems(const QVariantList &items);
    void setHistory(const QDate &through, History history);

private:
    void rebuild();
    QVariantList m_items;
    QDate m_through;
    History m_history;
};
