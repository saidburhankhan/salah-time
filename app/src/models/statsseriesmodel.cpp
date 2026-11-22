#include "statsseriesmodel.h"

StatsSeriesModel::StatsSeriesModel(QObject *parent) : TypedListModel(parent) {
    setRoleNames({{DayRole, "day"}, {PlannedRole, "planned"}, {DoneRole, "done"},
                  {MinutesRole, "minutes"}, {CountRole, "count"}});
}

QVariantList StatsSeriesModel::values(const Row &row) {
    return {row.day, row.planned, row.done, row.minutes, row.count};
}

void StatsSeriesModel::setRows(const QVector<Row> &rows) {
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
