#include "statsdailymodel.h"

#include <QHash>
#include <algorithm>

StatsDailyModel::StatsDailyModel(QObject *parent) : TypedListModel(parent) {
    setRoleNames({{DayRole, "day"}, {PlannedRole, "planned"}, {DoneRole, "done"},
                  {MinutesRole, "minutes"}, {CountRole, "count"}, {PrayersRole, "prayers"},
                  {TooltipRole, "tooltip"}});
}

QVariantList StatsDailyModel::values(const Row &row) {
    return {row.day, row.planned, row.done, row.minutes, row.count, row.prayers, row.tooltip};
}

void StatsDailyModel::setRows(const QVector<Row> &rows) {
    QStringList keys;
    QVector<QVariantList> data;
    keys.reserve(rows.size());
    data.reserve(rows.size());
    for (const Row &row : rows) {
        keys.append(row.day);
        data.append(values(row));
    }
    setRows(keys, data);
}

void StatsDailyModel::setStats(const Dto::Stats &stats, const QString &today,
                               const QString &selectedItemId) {
    QVector<Row> rows;

    if (selectedItemId.isEmpty()) {
        rows.reserve(int(stats.daily.size()));
        for (const Dto::StatsDaily &daily : stats.daily) {
            if (!today.isEmpty() && daily.day > today) continue;
            Row row;
            row.day = daily.day;
            row.planned = daily.json.value(QStringLiteral("planned")).toInt() > 0;
            row.done = daily.json.value(QStringLiteral("done")).toInt() > 0;
            row.minutes = daily.json.value(QStringLiteral("minutes")).toInt();
            row.count = daily.json.value(QStringLiteral("count")).toInt();
            row.prayers = daily.json.value(QStringLiteral("prayers")).toInt();
            row.tooltip = daily.tooltip;
            rows.append(row);
        }
    } else {
        QHash<QString, QString> tooltips;
        for (const Dto::StatsDaily &daily : stats.daily) tooltips.insert(daily.day, daily.tooltip);
        for (const Dto::StatsItem &item : stats.items) {
            if (item.id != selectedItemId) continue;
            rows.reserve(int(item.series.size()));
            for (const Dto::StatsSeries &series : item.series) {
                if (!today.isEmpty() && series.day > today) continue;
                Row row;
                row.day = series.day;
                row.planned = series.json.value(QStringLiteral("planned")).toBool();
                row.done = series.json.value(QStringLiteral("done")).toBool();
                row.minutes = series.json.value(QStringLiteral("minutes")).toInt();
                row.count = series.json.value(QStringLiteral("count")).toInt();
                row.tooltip = tooltips.value(series.day);
                rows.append(row);
            }
            break;
        }
    }

    setRows(rows);

    // The empty-state check follows the whole requested range, like the old
    // QML `stats.daily.some(...)` did, even when a task filter is active.
    bool active = false;
    for (const Dto::StatsDaily &daily : stats.daily) {
        if (daily.json.value(QStringLiteral("done")).toInt() > 0 ||
            daily.json.value(QStringLiteral("minutes")).toInt() > 0 ||
            daily.json.value(QStringLiteral("count")).toInt() > 0 ||
            daily.json.value(QStringLiteral("prayers")).toInt() > 0) {
            active = true;
            break;
        }
    }

    int planned = 0, done = 0, minutes = 0, count = 0, prayers = 0;
    int maxPlanned = 0, maxDone = 0, maxMinutes = 0, maxCount = 0;
    for (const Row &row : rows) {
        planned += row.planned ? 1 : 0;
        done += row.done ? 1 : 0;
        minutes += row.minutes;
        count += row.count;
        prayers += row.prayers;
        maxPlanned = std::max(maxPlanned, row.planned ? 1 : 0);
        maxDone = std::max(maxDone, row.done ? 1 : 0);
        maxMinutes = std::max(maxMinutes, row.minutes);
        maxCount = std::max(maxCount, row.count);
    }
    if (m_maxPlanned != maxPlanned || m_maxDone != maxDone || m_maxMinutes != maxMinutes ||
        m_maxCount != maxCount) {
        m_maxPlanned = maxPlanned;
        m_maxDone = maxDone;
        m_maxMinutes = maxMinutes;
        m_maxCount = maxCount;
        emit maxChanged();
    }
    m_shownTotals.set(planned, done, minutes, count, prayers, 0, rows.size() * 5,
                      rows.size());
    emit shownCountChanged();
    if (m_hasActivity != active) {
        m_hasActivity = active;
        emit hasActivityChanged();
    }
}
