#pragma once

#include "typedlistmodel.h"
#include "viewstate.h"
#include "../client/dto.h"

// Daily statistics rows shown by the charts and table. Rows are filtered to the
// selected task when one is active and to days up to today, exactly like the
// former QML `shownDaily()` helper.
class StatsDailyModel : public TypedListModel {
    Q_OBJECT
    Q_PROPERTY(StatsTotals *shownTotals READ shownTotals CONSTANT)
    Q_PROPERTY(int shownCount READ shownCount NOTIFY shownCountChanged)
    Q_PROPERTY(bool hasActivity READ hasActivity NOTIFY hasActivityChanged)
    Q_PROPERTY(int maxPlanned READ maxPlanned NOTIFY maxChanged)
    Q_PROPERTY(int maxDone READ maxDone NOTIFY maxChanged)
    Q_PROPERTY(int maxMinutes READ maxMinutes NOTIFY maxChanged)
    Q_PROPERTY(int maxCount READ maxCount NOTIFY maxChanged)
public:
    enum Role {
        DayRole = Qt::UserRole + 1,
        PlannedRole,
        DoneRole,
        MinutesRole,
        CountRole,
        PrayersRole,
        TooltipRole
    };

    struct Row {
        QString day;
        bool planned = false;
        bool done = false;
        int minutes = 0;
        int count = 0;
        int prayers = 0;
        QString tooltip;
    };

    explicit StatsDailyModel(QObject *parent = nullptr);

    StatsTotals *shownTotals() { return &m_shownTotals; }
    int shownCount() const { return rowCount(); }
    bool hasActivity() const { return m_hasActivity; }
    int maxPlanned() const { return m_maxPlanned; }
    int maxDone() const { return m_maxDone; }
    int maxMinutes() const { return m_maxMinutes; }
    int maxCount() const { return m_maxCount; }

    using TypedListModel::setRows;
    void setRows(const QVector<Row> &rows);
    void setStats(const Dto::Stats &stats, const QString &today, const QString &selectedItemId);

signals:
    void shownCountChanged();
    void hasActivityChanged();
    void maxChanged();

private:
    static QVariantList values(const Row &row);

    StatsTotals m_shownTotals;
    bool m_hasActivity = false;
    int m_maxPlanned = 0, m_maxDone = 0, m_maxMinutes = 0, m_maxCount = 0;
};
