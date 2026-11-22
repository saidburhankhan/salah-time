#include "prayermodel.h"

PrayerModel::PrayerModel(QObject *parent) : TypedListModel(parent) {
    setRoleNames({{PrayerIdRole, "prayerId"}, {NameRole, "name"}, {TimeRole, "time"},
                  {PassedRole, "passed"}, {CurrentRole, "current"}, {MarkedRole, "marked"},
                  {PendingRole, "pending"}, {VersionRole, "version"}, {DayRole, "day"}});
}

QVariantList PrayerModel::values(const Row &row) {
    return {row.prayerId, row.name, row.time, row.passed, row.current, row.marked,
            row.pending, row.version, row.day};
}

void PrayerModel::setRows(const QVector<Row> &rows) {
    QStringList keys;
    QVector<QVariantList> data;
    keys.reserve(rows.size());
    data.reserve(rows.size());
    for (const Row &row : rows) {
        keys.append(row.prayerId);
        data.append(values(row));
    }
    setRows(keys, data);
}
